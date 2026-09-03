import React, { useEffect, useState } from "react";
import { AppRectangle, Global, GlobalContextType, Log } from "./interfaces/global";
import AppTabs from "./AppTabs";
import "./App.css";
import { Window } from "@tauri-apps/api/window";
import { debug } from "@tauri-apps/plugin-log";
import { listen } from "@tauri-apps/api/event";
import { DateTime } from "ts-luxon";
import { invoke } from "@tauri-apps/api/core";

const defaultGlobal = {
  // Devices
  keyboards: [],
  mouses: [],
  // Logging
  maximum_logs: 250,
  logs: [],
  // Settings
  theme: "Dark",
  default_width: 800,
  default_height: 600,
  zoom: 100,
  enable_clipboard: true,
  auto_connect: true,
  download_path: "",
  remembered_keyboards: [],
  remembered_mouses: [],
  // Others
  isDragActive: false,
  dragAppId: '',
  dragPaths: [],
  dragAppRectangles: [],
};
export const GlobalContext = React.createContext<GlobalContextType>(null as unknown as GlobalContextType);

function App() {
  const [global, setGlobalState] = useState<Global>(defaultGlobal);

  const setGlobal = (newGlobal: Partial<Global>) => {
    setGlobalState((previous) => ({ ...previous, ...newGlobal }));
  };

  /* Logs are handled globally */
  useEffect(() => {
    if (global.logs.length > global.maximum_logs) {
      setGlobal({
        logs: global.logs.slice(-global.maximum_logs),
      });
    }
  }, [global.logs, global.maximum_logs]);
  useEffect(() => {
    const unlisten = listen<Log>("backend-add-log", (event) => {
      const content = event.payload.message;
      const level = event.payload.level;
      addLog(content, level);
    });

    return () => {
      unlisten.then(f => f());
    };
  }, []);
  const newTimestamp = (): string => {
    return DateTime.local().toFormat('yyyy-LL-dd HH:mm:ss');
  };
  const addLog = (log: string, level: number): void => {
    setGlobalState((previous) => ({
      ...previous,
      logs: previous.logs.concat({
        tag: newTimestamp(),
        message: log,
        level,
      })
    }));
  }

  /* Configuration update is handled globally */
  useEffect(() => {
    const unlisten = listen("backend-update-configuration", (event) => {
      const config = event.payload as any;
      setConfig(config);
    });

    // Consider frontend to be ready
    invoke("frontend_ready");

    return () => {
      unlisten.then(f => f());
    };
  }, []);
  const setConfig = (config: Partial<Global>): void => {
    setGlobalState((previous) => ({
      ...previous,
      ...config,
    }));
  }

  function getHoveredAppId(x: number, y: number, appRectangles: AppRectangle[]): string {
    for (const app of appRectangles) {
      if (app.x1 < x && x < app.x2 && app.y1 < y && y < app.y2) {
        return app.id;
      }
    }
    return '';
  }

  useEffect(() => {
    const unlisten = Window.getCurrent().onDragDropEvent((event) => {
      if (event.payload.type === 'over') {
        const dragAppId = getHoveredAppId(event.payload.position.x, event.payload.position.y, global.dragAppRectangles);
        setGlobalState((previous) => ({
          ...previous,
          isDragActive: true,
          dragAppId,
          dragPaths: [],
        }));
      } else if (event.payload.type === 'drop') {
        let paths = event.payload.paths;
        setGlobalState((previous) => ({
          ...previous,
          isDragActive: false,
          dragPaths: paths,
        }));
      } else {
        setGlobalState((previous) => ({
          ...previous,
          isDragActive: false,
          dragPaths: [],
        }));
      }
    });

    return () => {
      unlisten.then(f => f());
    };
  }, [global.dragAppRectangles]);
  useEffect(() => {
    debug(`invoke transfer_files`);
    if (global.dragAppId && global.dragPaths.length > 0) {
      invoke("transfer_files", { drag: JSON.stringify({ app_id: global.dragAppId , paths: global.dragPaths }) });
      setGlobal({ dragAppId: '', dragPaths: [] });
    }
  }, [global.dragAppId, global.dragPaths.length]);

  return (
    <main className="container">
      <GlobalContext.Provider
        value={{...global, setGlobal}}
      >
        <AppTabs />
      </GlobalContext.Provider>
    </main>
  );
}

export default App;
