import { useEffect, useState } from "react";
import "./App.css";
import PhotoGallery from "./PhotoGallery";
import { invoke } from "@tauri-apps/api/core";

function App() {
  const [itemCount, setItemCount] = useState(0);

  useEffect(() => {
    invoke<number>("get_total_image_count")
      .then((count) => {
        console.log("Total image count:", count);
        setItemCount(count);
      })
      .catch((err) => console.error("Failed to get image count:", err));
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
      {itemCount > 0 ? (
        <PhotoGallery itemsCount={itemCount} columnCount={5} />
      ) : (
        <div>Loading gallery...</div>
      )}
    </main>
  );
}

export default App;
