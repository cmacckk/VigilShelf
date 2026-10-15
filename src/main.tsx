import { createRoot } from "react-dom/client";
import App from "./App";

// 刻意不包 StrictMode：它会让副作用在开发模式下跑两遍，
// 而这里的副作用包括「注册全局拖放监听」和「挂起热区自动收起」两件系统级的事。
createRoot(document.getElementById("root")!).render(<App />);
