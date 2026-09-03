import { useCallback, useContext, useEffect, useRef, useState } from "react";
import { unstable_PasswordToggleField as PasswordToggleField } from "radix-ui";
import * as Collapsible from "@radix-ui/react-collapsible";
import { ChevronDownIcon, ChevronUpIcon, ClipboardCopyIcon, Cross2Icon, EyeClosedIcon, EyeOpenIcon, Pencil2Icon } from "@radix-ui/react-icons";
import { debug } from "@tauri-apps/plugin-log";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { platform } from "@tauri-apps/plugin-os";
import "./Home.css";
import WarnText from "../components/Warn";
import Monitors from "../components/Monitors";
import { App, AppRectangle, BorderPair, Global } from "../interfaces/global";
import { GlobalContext } from "../App";

function Home() {
  const global = useContext(GlobalContext);
  const [borders, setBorders] = useState<BorderPair[]>([]);
  const [selfInfo, setSelfInfo] = useState<App>({
    name: '',
    password: '',
    id: '',
    address_infos: [],
    set_of_monitors: {
      offset_x: 0,
      offset_y: 0,
      monitors: [],
    },
    online: true,
    focused_id: '',
    authorized_by_self: true,
    authorized_by_peer: false,
    file_transfers: [],
  });
  const [editSelfNetworkConfig, setEditSelfNetworkConfig] = useState<boolean>(false);
  const [editSelfName, setEditSelfName] = useState<string>('');
  const [editSelfPassword, setEditSelfPassword] = useState<string>('');

  const [discoveredApps, setDiscoveredApps] = useState<App[]>([]);
  const [editedApp, setEditedApp] = useState<string | null>(null);
  const [editAppPassword, setEditAppPassword] = useState<string>('');

  const refreshSelfApp = async () => {
    debug(`invoke refresh_self_app`);
    invoke("refresh_self_app");
  };

  const refreshDiscoveredApps= () => {
    debug(`invoke refresh_discovered_apps`);
    invoke("refresh_discovered_apps");
  };

  const appsRefs = useRef<HTMLDivElement[]>([]);
  appsRefs.current = [];
  const setAppsRef = useCallback((div: HTMLDivElement | null, index: number) => {
    if (div && appsRefs.current[index] !== div) {
      appsRefs.current[index] = div;
    }
  }, []);
  useEffect(() => {
    const dragAppRectangles = discoveredApps.map((app, index) => {
      if (!app.authorized_by_self || !app.authorized_by_peer) {
        return null;
      }
      const rectangle = appsRefs.current[index]?.getBoundingClientRect();
      if (rectangle) {
        if (platform() === 'macos') {
          const macOSTitleBarYOffset = 30; // Heuristic, I do not know a way to get title bar height
          return {
            id: app.id,
            x1: rectangle.left,
            y1: macOSTitleBarYOffset + rectangle.top,
            x2: rectangle.right,
            y2: macOSTitleBarYOffset + rectangle.bottom,
          };
        }
        // Using ratios to work with webviews on linux
        const horizontalRatio = window.devicePixelRatio * window.outerWidth / window.innerWidth;
        const verticalRatio = window.devicePixelRatio * window.outerHeight / window.innerHeight;
        return {
          id: app.id,
          x1: rectangle.left * horizontalRatio,
          y1: rectangle.top * verticalRatio,
          x2: rectangle.right * horizontalRatio,
          y2: rectangle.bottom * verticalRatio,
        };
      }
      return null;
    })
      .filter((app) => app) as unknown as AppRectangle[];
    global.setGlobal({ dragAppRectangles });
  }, [discoveredApps, global.isDragActive]);

  useEffect(() => {
    const unlisten = listen<{ borders: BorderPair[] }>("to-frontend-update-borders", (event) => {
      const borders = event.payload.borders;
      setBorders(borders);
    });
    return () => {
      unlisten.then(f => f());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<App>("to-frontend-update-self-app", (event) => {
      // No debug, to prevent password from showing in the logs
      // debug(`to-frontend-update-self-app event: ${JSON.stringify(event.payload)}`);
      const app = event.payload;
      setSelfInfo(app);
    });

    // Refresh does not work well when the code is sync
    new Promise(f => { setTimeout(f, 0); }).then(() => {
      refreshSelfApp();
    });

    return () => {
      unlisten.then(f => f());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<App[]>("to-frontend-update-discovered-apps", (event) => {
      // No debug, to prevent password from showing in the logs
      // debug(`to-frontend-update-discovered-apps event: ${JSON.stringify(event.payload)} ${event.payload.length}`);
      const apps = event.payload;
      setDiscoveredApps(apps);
    });

    // Refresh does not work well when the code is sync
    new Promise(f => { setTimeout(f, 0); }).then(() => {
      refreshDiscoveredApps();
    });

    return () => {
      unlisten.then(f => f());
    };
  }, []);

  const submitNetworkConfig = (name: string, password: string) => {
    debug(`invoke submit_network_config`);
    invoke("submit_config", { partial_config: JSON.stringify({ app_name: name, password }) });
    setEditSelfNetworkConfig(false);
  };

  const submitAppConfig = (id: string, password: string) => {
    debug(`invoke submit_app_network_config`);
    invoke("submit_app_network_config", { config: JSON.stringify({ id, password }) });
    setEditedApp(null);
  };

  const requestClipboard = (peer_id: string) => {
    debug(`invoke request_clipboard`);
    invoke("request_clipboard", { peer_id });
  };

  const setSelfOnline = (online: boolean) => {
    debug(`invoke set_self_online`);
    invoke("set_self_online", { online: JSON.stringify({ online }) });
  };

  const disconnectFromApp = (app: App) => {
    debug(`invoke disconnect_from_app`);
    invoke("disconnect_from_app", { app: JSON.stringify({ id: app.id }) });
  };

  const connectToApp = (app: App, password: string) => {
    debug(`invoke connect_to_app`);
    invoke("connect_to_app", { app: JSON.stringify({ id: app.id, password }) });
  };

  const isConnected = (app: App): boolean => {
    return app.authorized_by_self && app.authorized_by_peer;
  };

  const shouldShowClipboard = (global: Global, app: App): boolean => {
    return global.enable_clipboard && isConnected(app);
  };

  const isNameUnique = (app: App, allApps: App[]): boolean => {
    let nameCount = 0;
    for (const otherApp of allApps) {
      if (app.name === otherApp.name) {
        nameCount += 1;
      }
      if (nameCount > 1) {
        return false;
      }
    }
    return true;
  };

  const abbreviateId = (id: string): string => {
    const ID_ABBREVIATON_PREFIX_LENGTH = 4;
    const ID_ABBREVIATON_SUFFIX_LENGTH = 4;
    return `${id.substring(0, ID_ABBREVIATON_PREFIX_LENGTH)}..${id.substring(id.length - ID_ABBREVIATON_SUFFIX_LENGTH)}`;
  };

  /* The lamp is the only place state is shown as colour, so its meaning stays learnable. */
  const getLampClassName = (app: App): string => {
    if (isConnected(app)) {
      return 'lamp lamp-connected';
    }
    if (app.online) {
      return 'lamp lamp-online';
    }
    return 'lamp lamp-offline';
  };

  const getStatusText = (app: App): string => {
    if (app.authorized_by_self && app.authorized_by_peer) {
      return 'Connected';
    }
    if (!app.online && !app.authorized_by_self && !app.authorized_by_peer) {
      return 'Offline';
    }
    if (!app.authorized_by_self && app.authorized_by_peer) {
      return 'Waiting for you to connect';
    }
    if (app.authorized_by_self && !app.authorized_by_peer) {
      return 'Connecting';
    }
    if (app.online) {
      return 'Available';
    }
    return 'Not connected';
  };

  const connectedCount = discoveredApps.filter(isConnected).length;

  return (
    <div className="home">
      {/* ---------- This machine ---------- */}
      <div className="eyebrow">This machine</div>

      <Collapsible.Root className="card machine-card machine-card-self">
        <div className="machine-row">
          <div className="machine-identity">
            <div className="machine-name-line">
              <span className="machine-name">{selfInfo.name || 'Starting up'}</span>
              {!isNameUnique(selfInfo, [selfInfo, ...discoveredApps]) && (
                <span className="mono machine-id-hint">{abbreviateId(selfInfo.id)}</span>
              )}
              <Collapsible.Trigger className="icon-button collapsible-arrow collapsible-arrow-collapsed" aria-label="Show network details">
                <ChevronDownIcon />
              </Collapsible.Trigger>
              <Collapsible.Trigger className="icon-button collapsible-arrow collapsible-arrow-opened" aria-label="Hide network details">
                <ChevronUpIcon />
              </Collapsible.Trigger>
            </div>
            <div className="machine-status">
              <span className={selfInfo.online ? 'lamp lamp-connected' : 'lamp lamp-offline'} />
              <span>{selfInfo.online ? 'Accepting connections' : 'Blocking incoming connections'}</span>
            </div>
          </div>

          <div className="machine-actions">
            <button onClick={() => setSelfOnline(!selfInfo.online)}>
              {selfInfo.online ? 'Block incoming connections' : 'Accept connections'}
            </button>
            {!editSelfNetworkConfig ? (
              <button
                className="icon-button"
                aria-label="Edit name and password"
                title="Edit name and password"
                onClick={() => { setEditSelfName(selfInfo.name); setEditSelfPassword(selfInfo.password); setEditSelfNetworkConfig(true); }}
              >
                <Pencil2Icon />
              </button>
            ) : (
              <button
                className="icon-button"
                aria-label="Discard changes"
                title="Discard changes"
                onClick={() => setEditSelfNetworkConfig(false)}
              >
                <Cross2Icon />
              </button>
            )}
          </div>
        </div>

        {editSelfNetworkConfig && (
          <div className="machine-editor">
            <div className="form-element">
              <label className="form-label" htmlFor="self-name">Name other machines see</label>
              <input id="self-name" type="name" value={editSelfName} onChange={(e) => setEditSelfName(e.target.value)} />
            </div>
            <div className="form-element">
              <label className="form-label" htmlFor="self-password">Password</label>
              <PasswordToggleField.Root>
                <div className="password-field">
                  <PasswordToggleField.Input id="self-password" value={editSelfPassword} onChange={(e) => setEditSelfPassword(e.target.value)} />
                  <PasswordToggleField.Toggle className="icon-button" aria-label="Show or hide password">
                    <PasswordToggleField.Icon
                      visible={<EyeOpenIcon />}
                      hidden={<EyeClosedIcon />}
                    />
                  </PasswordToggleField.Toggle>
                </div>
              </PasswordToggleField.Root>
              <WarnText show={editSelfPassword.length === 0} text="No password. Only do this if you trust every machine on this network." />
              <WarnText show={editSelfPassword.length >= 1 && editSelfPassword.length <= 10} text="Short password. Use more than 10 characters." />
            </div>
            <button className="primary form-submit-button" onClick={() => submitNetworkConfig(editSelfName, editSelfPassword)}>
              Save changes
            </button>
          </div>
        )}

        <Collapsible.Content className="machine-details">
          <dl className="detail-list">
            <dt>Peer id</dt>
            <dd className="mono">{selfInfo.id || '—'}</dd>
            <dt>Addresses</dt>
            <dd className="mono">{selfInfo.address_infos.length > 0 ? selfInfo.address_infos.join('  ·  ') : '—'}</dd>
          </dl>
        </Collapsible.Content>
      </Collapsible.Root>

      {/* ---------- Other machines ---------- */}
      <div className="eyebrow">
        On this network
        <span className="eyebrow-count">
          {discoveredApps.length > 0 ? `${connectedCount}/${discoveredApps.length} connected` : '0 found'}
        </span>
      </div>

      {discoveredApps.length === 0 ? (
        <div className="empty-state">
          <p>
            No other machine found yet. Open UniversalKVM on another computer on the same
            local network and it appears here.
          </p>
        </div>
      ) : (
        <div className="machine-list">
          {discoveredApps.map((app, appIndex) => (
            <Collapsible.Root
              key={app.id}
              className={`card machine-card${global.isDragActive && isConnected(app) ? (global.dragAppId === app.id ? ' drop-target-active' : ' drop-target-ready') : ''}`}
            >
              <div
                className="machine-row"
                ref={(divElement) => setAppsRef(divElement, appIndex)}
              >
                <div
                  className="machine-identity"
                  style={{ pointerEvents: global.isDragActive ? 'none' : 'inherit' }}
                >
                  <div className="machine-name-line">
                    <span className="machine-name">{app.name}</span>
                    {!isNameUnique(app, [selfInfo, ...discoveredApps]) && (
                      <span className="mono machine-id-hint">{abbreviateId(app.id)}</span>
                    )}
                    <Collapsible.Trigger className="icon-button collapsible-arrow collapsible-arrow-collapsed" aria-label="Show network details">
                      <ChevronDownIcon />
                    </Collapsible.Trigger>
                    <Collapsible.Trigger className="icon-button collapsible-arrow collapsible-arrow-opened" aria-label="Hide network details">
                      <ChevronUpIcon />
                    </Collapsible.Trigger>
                  </div>
                  <div className="machine-status">
                    <span className={getLampClassName(app)} />
                    <span>{getStatusText(app)}</span>
                    {shouldShowClipboard(global, app) && (
                      <button
                        className="icon-button"
                        aria-label={`Copy clipboard from ${app.name}`}
                        title={`Copy clipboard from ${app.name}`}
                        onClick={() => requestClipboard(app.id)}
                      >
                        <ClipboardCopyIcon />
                      </button>
                    )}
                  </div>
                  {app.file_transfers.map((fileTransfers, transferIndex) => (
                    <div className="transfer" key={transferIndex}>
                      <div className="transfer-track">
                        <div
                          className="transfer-fill"
                          style={{ width: `${100 * fileTransfers.finished_bytes / (fileTransfers.total_bytes + 1)}%` }}
                        />
                      </div>
                      <span className="mono">
                        {(100 * fileTransfers.finished_bytes / (fileTransfers.total_bytes + 1)).toFixed(0)}% of {Math.ceil(fileTransfers.total_bytes / 1024 / 1024)} MB
                      </span>
                    </div>
                  ))}
                </div>

                <div className="machine-actions">
                  {app.authorized_by_self && (
                    <button onClick={() => disconnectFromApp(app)}>Disconnect</button>
                  )}
                  {app.online && !app.authorized_by_self && (
                    <button className="primary" onClick={() => connectToApp(app, app.password)}>Connect</button>
                  )}
                  {editedApp !== app.id ? (
                    <button
                      className="icon-button"
                      aria-label={`Edit password for ${app.name}`}
                      title={`Edit password for ${app.name}`}
                      onClick={() => { setEditAppPassword(app.password); setEditedApp(app.id); }}
                    >
                      <Pencil2Icon />
                    </button>
                  ) : (
                    <button
                      className="icon-button"
                      aria-label="Discard changes"
                      title="Discard changes"
                      onClick={() => setEditedApp(null)}
                    >
                      <Cross2Icon />
                    </button>
                  )}
                </div>
              </div>

              {editedApp === app.id && (
                <div className="machine-editor">
                  <div className="form-element">
                    <label className="form-label" htmlFor={`password-${app.id}`}>Password for {app.name}</label>
                    <PasswordToggleField.Root>
                      <div className="password-field">
                        <PasswordToggleField.Input id={`password-${app.id}`} value={editAppPassword} onChange={(e) => setEditAppPassword(e.target.value)} />
                        <PasswordToggleField.Toggle className="icon-button" aria-label="Show or hide password">
                          <PasswordToggleField.Icon
                            visible={<EyeOpenIcon />}
                            hidden={<EyeClosedIcon />}
                          />
                        </PasswordToggleField.Toggle>
                      </div>
                    </PasswordToggleField.Root>
                  </div>
                  <button className="primary form-submit-button" onClick={() => submitAppConfig(app.id, editAppPassword)}>
                    Save password
                  </button>
                </div>
              )}

              <Collapsible.Content className="machine-details">
                <dl className="detail-list">
                  <dt>Peer id</dt>
                  <dd className="mono">{app.id}</dd>
                  <dt>Addresses</dt>
                  <dd className="mono">{app.address_infos.length > 0 ? app.address_infos.join('  ·  ') : '—'}</dd>
                </dl>
              </Collapsible.Content>
            </Collapsible.Root>
          ))}
        </div>
      )}

      {/* ---------- Monitor layout ---------- */}
      <Monitors
        appsMonitors={[{
          name: selfInfo.name,
          id: selfInfo.id,
          offset_x: selfInfo.set_of_monitors.offset_x,
          offset_y: selfInfo.set_of_monitors.offset_y,
          monitors: selfInfo.set_of_monitors.monitors
        }].concat(discoveredApps
          .filter(isConnected)
          .map((app => ({
            name: app.name,
            id: app.id,
            offset_x: app.set_of_monitors.offset_x,
            offset_y: app.set_of_monitors.offset_y,
            monitors: app.set_of_monitors.monitors
          })))
        )}
        borders={borders}
        focusedId={selfInfo.focused_id}
      />
    </div>
  );
}

export default Home;
