export type Photo = {
  id: string;
  thumbnailPath: string;
  originalPath: string;
};

export type PersonCluster = {
  id: string;
  thumbnail_path: string;
  photo_count: number;
  detection_ids: string[];
};
