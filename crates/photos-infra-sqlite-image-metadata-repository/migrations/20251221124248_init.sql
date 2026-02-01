CREATE TABLE image
(
    image_uuid             BLOB PRIMARY KEY,
    image_format_id        INTEGER NOT NULL,
    image_exif_timestamp   TIMESTAMP,
    image_os_timestamp     TIMESTAMP NOT NULL,
    image_import_timestamp TIMESTAMP NOT NULL,
    image_is_analyzed      INTEGER   DEFAULT 0
);

CREATE TABLE face_detection
(
    fd_uuid       BLOB PRIMARY KEY,
    image_uuid    BLOB    NOT NULL,
    fd_roi_x      INTEGER NOT NULL CHECK (fd_roi_x >= 0),
    fd_roi_y      INTEGER NOT NULL CHECK (fd_roi_y >= 0),
    fd_roi_w      INTEGER NOT NULL CHECK (fd_roi_w > 0),
    fd_roi_h      INTEGER NOT NULL CHECK (fd_roi_h > 0),
    fd_confidence REAL    NOT NULL CHECK (fd_confidence BETWEEN 0.0 AND 1.0),
    fd_embedding  BLOB      DEFAULT NULL,
    face_uuid     BLOB      DEFAULT NULL,
    fd_created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (image_uuid) REFERENCES image (image_uuid) ON DELETE CASCADE,
    FOREIGN KEY (face_uuid) REFERENCES face (face_uuid) ON DELETE SET NULL
);

CREATE TABLE face
(
    face_uuid BLOB PRIMARY KEY,
    face_name TEXT
);
