import { useEffect, useState, useCallback } from "react";
import "./App.css";
import PhotoGallery from "./PhotoGallery";
import { invoke } from "@tauri-apps/api/core";
import { SingleImageViewer } from "./SingleImageViewer"; // Import the new component

function App() {
  const [itemCount, setItemCount] = useState(0);
  const [selectedImageIndex, setSelectedImageIndex] = useState<number | null>(
    null,
  ); // New state for selected image

  useEffect(() => {
    invoke<number>("get_total_image_count")
      .then((count) => {
        console.log("Total image count:", count);
        setItemCount(count);
      })
      .catch((err) => console.error("Failed to get image count:", err));
  }, []);

  const handleImageClick = useCallback((index: number) => {
    setSelectedImageIndex(index);
  }, []);

  const handleCloseImageViewer = useCallback(() => {
    setSelectedImageIndex(null);
  }, []);

  return (
    <main
      className="container"
      style={{
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
        width: "100%",
        height: "100vh",
        overflow: "hidden",
      }}
    >
      {selectedImageIndex !== null ? (
        // Display single image viewer if an image is selected
        <SingleImageViewer
          imageIndex={selectedImageIndex}
          onClose={handleCloseImageViewer}
        />
      ) : itemCount > 0 ? (
        // Otherwise, display the gallery
        <PhotoGallery
          itemsCount={itemCount}
          onImageClick={handleImageClick} // Pass the click handler
        />
      ) : (
        <div>Loading gallery...</div>
      )}
    </main>
  );
}

export default App;
