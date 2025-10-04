use anyhow::Result;
use block2::StackBlock;
use candle_core::{Error as CandleError, Tensor};
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_core_ml::{
    MLDictionaryFeatureProvider, MLFeatureProvider, MLMultiArray, MLMultiArrayDataType,
};
use objc2_foundation::{NSArray, NSNumber, NSString};

/// Converts a Candle Tensor to a Core ML MLMultiArray (zero-copy optimized version)
pub(crate) fn tensor_to_mlmultiarray(
    tensor: &Tensor,
) -> Result<Retained<MLMultiArray>, CandleError> {
    let contiguous_tensor = if tensor.is_contiguous() {
        tensor.clone()
    } else {
        tensor.contiguous()?
    };

    let element_count = tensor.elem_count();
    let dims = tensor.dims();
    let mut shape = Vec::with_capacity(dims.len());
    for &dim in dims {
        shape.push(NSNumber::new_usize(dim));
    }
    let shape_nsarray = NSArray::from_retained_slice(&shape);

    let multi_array_result = unsafe {
        MLMultiArray::initWithShape_dataType_error(
            MLMultiArray::alloc(),
            &shape_nsarray,
            MLMultiArrayDataType::Float32,
        )
    };

    match multi_array_result {
        Ok(ml_array) => {
            use std::sync::atomic::{AtomicBool, Ordering};
            let copied = AtomicBool::new(false);

            let flattened_tensor = contiguous_tensor.flatten_all()?;

            // Use Candle's to_vec1 but keep the optimization pattern
            let data_vec = flattened_tensor.to_vec1::<f32>()?;

            unsafe {
                ml_array.getMutableBytesWithHandler(&StackBlock::new(
                    |ptr: std::ptr::NonNull<std::ffi::c_void>, len, _| {
                        let dst = ptr.as_ptr() as *mut f32;
                        let src = data_vec.as_ptr();
                        let copy_elements =
                            element_count.min(len as usize / std::mem::size_of::<f32>());

                        if copy_elements > 0
                            && len as usize >= copy_elements * std::mem::size_of::<f32>()
                        {
                            std::ptr::copy_nonoverlapping(src, dst, copy_elements);
                            copied.store(true, Ordering::Relaxed);
                        }
                    },
                ));
            }

            if copied.load(Ordering::Relaxed) {
                Ok(ml_array)
            } else {
                Err(CandleError::Msg(
                    "Failed to copy data to MLMultiArray".to_string(),
                ))
            }
        }
        Err(err) => Err(CandleError::Msg(format!(
            "Failed to create MLMultiArray: {:?}",
            err
        ))),
    }
}

/// Creates a proper MLDictionaryFeatureProvider for model input
pub(crate) fn create_feature_provider(
    input_name: &str,
    input_array: &MLMultiArray,
) -> Result<Retained<MLDictionaryFeatureProvider>> {
    use objc2::runtime::AnyObject;
    use objc2_core_ml::{MLDictionaryFeatureProvider, MLFeatureValue};
    use objc2_foundation::{NSDictionary, NSString};

    objc2::rc::autoreleasepool(|_| {
        // Key and value
        let key = NSString::from_str(input_name); // Retained<NSString>
        let value = unsafe { MLFeatureValue::featureValueWithMultiArray(input_array) };

        // Build single-pair dictionary
        let dict: Retained<NSDictionary<NSString, AnyObject>> =
            NSDictionary::from_slices::<NSString>(&[&*key], &[&*value]);

        // Create the provider
        unsafe {
            MLDictionaryFeatureProvider::initWithDictionary_error(
                MLDictionaryFeatureProvider::alloc(),
                dict.as_ref(),
            )
        }
        .map_err(|e| e.into())
    })
}

pub(crate) fn extract_vector(
    prediction: &ProtocolObject<dyn MLFeatureProvider>,
    output_name: &str,
) -> Result<Vec<f32>> {
    objc2::rc::autoreleasepool(|_| unsafe {
        let name = NSString::from_str(output_name);
        let value = prediction
            .featureValueForName(&name)
            .ok_or_else(|| anyhow::anyhow!("Output '{}' not found", output_name))?;

        let marray = value
            .multiArrayValue()
            .ok_or_else(|| anyhow::anyhow!("Output '{}' is not MLMultiArray", output_name))?;

        let count = marray.count() as usize;
        let mut buf = vec![0.0f32; count];

        // Use a cell pattern to allow mutation in the Fn closure
        use std::cell::RefCell;
        let buf_cell = RefCell::new(&mut buf);

        marray.getBytesWithHandler(&block2::StackBlock::new(
            |ptr: std::ptr::NonNull<std::ffi::c_void>, len: isize| {
                let src = ptr.as_ptr() as *const f32;
                let copy_elements = count.min(len as usize / std::mem::size_of::<f32>());
                if copy_elements > 0
                    && len as usize >= copy_elements * std::mem::size_of::<f32>()
                    && let Ok(mut buf_ref) = buf_cell.try_borrow_mut()
                {
                    std::ptr::copy_nonoverlapping(src, buf_ref.as_mut_ptr(), copy_elements);
                }
            },
        ));

        Ok(buf)
    })
}
