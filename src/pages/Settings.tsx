import { useContext, useEffect, useState } from "react";
import { GlobalContext } from "../App";
import WarnText from "../components/Warn";
import "./Settings.css";
import { debug } from "@tauri-apps/plugin-log";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from '@tauri-apps/api/webview';


function Settings() {
  const global = useContext(GlobalContext);

  const [theme, setTheme] = useState<string>(global.theme);
  const [defaultWidth, setDefaultWidth] = useState<number>(global.default_width);
  const [defaultHeight, setDefaultHeight] = useState<number>(global.default_height);
  const [zoom, setZoom] = useState<number>(global.zoom);
  const [enableClipboard, setEnableClipboard] = useState<boolean>(global.enable_clipboard);
  const [autoConnect, setAutoConnect] = useState<boolean>(global.auto_connect);
  const [downloadPath, setDownloadPath] = useState<string>(global.download_path);

  const [configPath, setConfigPath] = useState<string>('');
  useEffect(() => {
    const fetchData = async () => {
      const configPath = await invoke<string>("get_config_path");
      setConfigPath(configPath);
    };

    fetchData();
  }, []);

  const MIN_SIZE = 200;

  useEffect(() => {
    setTheme(global.theme);
  }, [global.theme]);
  useEffect(() => {
    setDefaultWidth(global.default_width);
  }, [global.default_width]);
  useEffect(() => {
    setDefaultHeight(global.default_height);
  }, [global.default_height]);
  useEffect(() => {
    setZoom(global.zoom);
  }, [global.zoom]);
  useEffect(() => {
    setEnableClipboard(global.enable_clipboard);
  }, [global.enable_clipboard]);
  useEffect(() => {
    setAutoConnect(global.auto_connect);
  }, [global.auto_connect]);
  useEffect(() => {
    setDownloadPath(global.download_path);
  }, [global.download_path]);

  const onChangeTheme = (event: React.ChangeEvent<HTMLSelectElement>) => {
    event.preventDefault();
    setTheme(event.target.value);
    debug(`invoke submit_config (theme)`);
    invoke("submit_config", { partial_config: JSON.stringify({ theme: event.target.value }) });
  };

  const onChangeDefaultWidth = (event: React.ChangeEvent<HTMLInputElement>) => {
    event.preventDefault();
    let { value, min, max } = event.target;
    const width = Math.max(Number(min), Math.min(Number(max), Number(value)));
    setDefaultWidth(width);
  };

  const onSubmitDefaultWidth = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (defaultWidth >= MIN_SIZE) {
      debug(`invoke submit_config (default_width)`);
      invoke("submit_config", { partial_config: JSON.stringify({ default_width: defaultWidth }) });
    }
  };

  const onChangeDefaultHeight = (event: React.ChangeEvent<HTMLInputElement>) => {
    event.preventDefault();
    let { value, min, max } = event.target;
    const height = Math.max(Number(min), Math.min(Number(max), Number(value)));
    setDefaultHeight(height);
  };

  const onSubmitDefaultHeight = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (defaultHeight >= MIN_SIZE) {
      debug(`invoke submit_config (default_height)`);
      invoke("submit_config", { partial_config: JSON.stringify({ default_height: defaultHeight }) });
    }
  };

  const onChangeZoom = (event: React.ChangeEvent<HTMLInputElement>) => {
    event.preventDefault();
    let { value, min, max } = event.target;
    const zoom = Math.max(Number(min), Math.min(Number(max), Number(value)));
    setZoom(zoom);
  };

  const onSubmitZoom = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (zoom >= 25 && zoom <= 1000) {
      debug(`invoke submit_config (zoom)`);
      getCurrentWebview().setZoom(zoom / 100.0);
      invoke("submit_config", { partial_config: JSON.stringify({ zoom }) });
    }
  };

  const onChangeEnableClipboard = (event: React.ChangeEvent<HTMLInputElement>) => {
    event.preventDefault();
    setEnableClipboard(event.target.checked);
    debug(`invoke submit_config (enable clipboard)`);
    invoke("submit_config", { partial_config: JSON.stringify({ enable_clipboard: event.target.checked }) });
  };

  const onChangeAutoConnect = (event: React.ChangeEvent<HTMLInputElement>) => {
    event.preventDefault();
    setAutoConnect(event.target.checked);
    debug(`invoke submit_config (auto connect)`);
    invoke("submit_config", { partial_config: JSON.stringify({ auto_connect: event.target.checked }) });
  };

  const onChangeDownloadPath = (event: React.ChangeEvent<HTMLInputElement>) => {
    event.preventDefault();
    setDownloadPath(event.target.value);
  };

  const onSubmitDownloadPath = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    debug(`invoke submit_config (download path)`);
    invoke("submit_config", { partial_config: JSON.stringify({ download_path: downloadPath }) });
  };

  return (
    <div className="settings">
      <div className="eyebrow">Appearance</div>

      <form className="setting">
        <label className="setting-label" htmlFor="setting-theme">Theme</label>
        <div className="setting-control">
          <select name="theme" id="setting-theme" value={theme} onChange={onChangeTheme}>
            <option value="Dark">Dark</option>
            <option value="Light">Light</option>
          </select>
        </div>
      </form>

      <form onSubmit={onSubmitZoom} className="setting">
        <label className="setting-label" htmlFor="setting-zoom">Zoom</label>
        <div className="setting-control">
          <input
            id="setting-zoom"
            value={zoom}
            onChange={onChangeZoom}
            type="number"
            min={0}
            max={10000}
          />
          <span className="mono">%</span>
        </div>
        {zoom > 1000 && <WarnText show={zoom !== global.zoom} text="1000% is the maximum" />}
        {zoom <= 1000 && zoom >= 25 && <WarnText show={zoom !== global.zoom} text="Press Enter to apply" />}
        {zoom < 25 && <WarnText show={zoom !== global.zoom} text="25% is the minimum" />}
      </form>

      <div className="eyebrow">Window</div>

      <form onSubmit={onSubmitDefaultWidth} className="setting">
        <label className="setting-label" htmlFor="setting-width">Width at startup</label>
        <div className="setting-control">
          <input
            id="setting-width"
            value={defaultWidth}
            onChange={onChangeDefaultWidth}
            type="number"
            min={0}
            max={16384}
          />
          <span className="mono">px</span>
        </div>
        {defaultWidth >= MIN_SIZE
          ? <WarnText show={defaultWidth !== global.default_width} text="Press Enter to apply" />
          : <WarnText show={defaultWidth !== global.default_width} text={`${MIN_SIZE} px is the minimum`} />}
      </form>

      <form onSubmit={onSubmitDefaultHeight} className="setting">
        <label className="setting-label" htmlFor="setting-height">Height at startup</label>
        <div className="setting-control">
          <input
            id="setting-height"
            value={defaultHeight}
            onChange={onChangeDefaultHeight}
            type="number"
            min={0}
            max={16384}
          />
          <span className="mono">px</span>
        </div>
        {defaultHeight >= MIN_SIZE
          ? <WarnText show={defaultHeight !== global.default_height} text="Press Enter to apply" />
          : <WarnText show={defaultHeight !== global.default_height} text={`${MIN_SIZE} px is the minimum`} />}
      </form>

      <div className="eyebrow">Sharing</div>

      <form className="setting">
        <label className="setting-label" htmlFor="setting-clipboard">Share clipboard</label>
        <div className="setting-control">
          <input
            id="setting-clipboard"
            checked={enableClipboard}
            onChange={onChangeEnableClipboard}
            type="checkbox"
          />
        </div>
        <p className="setting-note">
          Copies the clipboard from the machine that had the cursor whenever the focus moves.
        </p>
      </form>

      <form className="setting">
        <label className="setting-label" htmlFor="setting-autoconnect">Reconnect automatically</label>
        <div className="setting-control">
          <input
            id="setting-autoconnect"
            checked={autoConnect}
            onChange={onChangeAutoConnect}
            type="checkbox"
          />
        </div>
        <p className="setting-note">
          Reconnects to machines you have already paired, without asking again.
        </p>
      </form>

      <form className="setting" onSubmit={onSubmitDownloadPath}>
        <label className="setting-label" htmlFor="setting-downloads">Received files go to</label>
        <div className="setting-control">
          <input
            id="setting-downloads"
            value={downloadPath}
            onChange={onChangeDownloadPath}
            type="text"
          />
        </div>
        <WarnText show={downloadPath !== global.download_path} text="Press Enter to apply" />
      </form>

      <div className="eyebrow">Storage</div>

      <div className="setting">
        <span className="setting-label">Settings folder</span>
        <div className="setting-readonly mono">{configPath || '—'}</div>
      </div>
    </div>
  );
}

export default Settings;
