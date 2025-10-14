import sys
from pathlib import Path
from PIL import Image
from tqdm import tqdm


def make_thumbnails(thumbnails: Path, originals: Path, sizes: list[int]):
    images = list(
        filter(
            lambda f: f.suffix.lower() in [".jpg", ".jpeg", ".png", ".bmp"],
            originals.iterdir(),
        )
    )
    thumbnails.mkdir(exist_ok=True)
    for size in sizes:
        subdir = thumbnails / str(size)
        subdir.mkdir(exist_ok=True)
        for image_path in tqdm(images):
            img = Image.open(image_path)
            img.thumbnail((size, size))
            img.save(subdir / image_path.name)


if __name__ == "__main__":
    path = Path(sys.argv[1])
    assert path.is_dir(), "library does not exist"
    originals = path / "originals"
    assert originals.is_dir(), "library should have an 'originals' subdirectory"
    thumbnails = path / "thumbnails"
    assert thumbnails.is_dir() or not thumbnails.exists(), (
        "'thumbnails' dir should not be a file"
    )
    make_thumbnails(thumbnails, originals, [32, 64, 128, 256])
