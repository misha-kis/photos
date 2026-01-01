CREATE TABLE IF NOT EXISTS image (
     uuid BLOB PRIMARY KEY,
     image_created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS face_detection (
    face_detection_id INTEGER PRIMARY KEY AUTOINCREMENT,
    image_uuid BLOB NOT NULL,
    roi_x1 INTEGER NOT NULL CHECK (roi_x1 >= 0),
    roi_y1 INTEGER NOT NULL CHECK (roi_y1 >= 0),
    roi_x2 INTEGER NOT NULL CHECK (roi_x2 > roi_x1),
    roi_y2 INTEGER NOT NULL CHECK (roi_y2 > roi_y1),
    embedding BLOB DEFAULT NULL,
    face_id INTEGER DEFAULT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (image_uuid) REFERENCES image(uuid) ON DELETE CASCADE ON UPDATE CASCADE
);