CREATE TABLE IF NOT EXISTS image
(
    uuid             BLOB PRIMARY KEY,
    format_id        INTEGER NOT NULL,
    is_analyzed      INTEGER   DEFAULT 0,
    image_created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS face_detection
(
    face_detection_id INTEGER PRIMARY KEY AUTOINCREMENT,
    image_uuid        BLOB    NOT NULL,
    roi_x             INTEGER NOT NULL CHECK (roi_x >= 0),
    roi_y             INTEGER NOT NULL CHECK (roi_y >= 0),
    roi_w             INTEGER NOT NULL CHECK (roi_w > 0),
    roi_h             INTEGER NOT NULL CHECK (roi_h > 0),
    confidence        REAL    NOT NULL CHECK ( confidence > 0 ),
    embedding         BLOB      DEFAULT NULL,
    face_id           INTEGER   DEFAULT NULL,
    created_at        TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (image_uuid) REFERENCES image (uuid) ON DELETE CASCADE ON UPDATE CASCADE
);