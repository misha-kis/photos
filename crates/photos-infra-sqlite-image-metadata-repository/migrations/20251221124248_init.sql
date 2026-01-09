CREATE TABLE image
(
    uuid             BLOB PRIMARY KEY,
    format_id        INTEGER NOT NULL,
    is_analyzed      INTEGER   DEFAULT 0,
    image_created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE face_detection
(
    uuid       BLOB PRIMARY KEY,
    image_uuid BLOB    NOT NULL,
    roi_x      INTEGER NOT NULL CHECK (roi_x >= 0),
    roi_y      INTEGER NOT NULL CHECK (roi_y >= 0),
    roi_w      INTEGER NOT NULL CHECK (roi_w > 0),
    roi_h      INTEGER NOT NULL CHECK (roi_h > 0),
    confidence REAL    NOT NULL CHECK (confidence BETWEEN 0.0 AND 1.0),
    embedding  BLOB      DEFAULT NULL,
    face_uuid  BLOB      DEFAULT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (image_uuid) REFERENCES image (uuid) ON DELETE CASCADE,
    FOREIGN KEY (face_uuid) REFERENCES face (uuid) ON DELETE SET NULL
);

CREATE TABLE face
(
    uuid BLOB PRIMARY KEY,
    name TEXT
);
