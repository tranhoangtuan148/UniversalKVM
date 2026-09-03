import { useContext, useEffect, useState } from "react";
import * as Collapsible from "@radix-ui/react-collapsible";
import * as Switch from "@radix-ui/react-switch";
import { ChevronDownIcon, ChevronUpIcon, ReloadIcon } from "@radix-ui/react-icons";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./Devices.css";
import { debug } from "@tauri-apps/plugin-log";
import { GlobalContext } from "../App";

interface KeyboardDevice {
  name: string,
  id: string,
  active: boolean,
}
interface MouseDevice {
  name: string,
  id: string,
  active: boolean,
}

/* Devices this app created to replay events. They cannot be captured. */
const VIRTUAL_KEYBOARD_SUFFIX = " Virtual Keyboard";
const VIRTUAL_MOUSE_SUFFIX = " Virtual Mouse";

function Devices() {

  const global = useContext(GlobalContext);

  const [keyboardDevices, setKeyboardDevices] = useState<KeyboardDevice[]>(global.keyboards);
  const [mouseDevices, setMouseDevices] = useState<MouseDevice[]>(global.mouses);

  const [latestBackendKeyboardUpdates, setLatestBackendKeyboardUpdates] = useState<KeyboardDevice[]>([]);
  const [latestBackendMouseUpdates, setLatestBackendMouseUpdates] = useState<MouseDevice[]>([]);

  useEffect(() => {
    if (JSON.stringify(latestBackendKeyboardUpdates) !== JSON.stringify(keyboardDevices)) {
      // debug(`to-frontend-update-keyboard-devices not identical: ${JSON.stringify(latestBackendKeyboardUpdates)} | ${JSON.stringify(keyboardDevices)}`);
      setKeyboardDevices(latestBackendKeyboardUpdates);
    } else {
      // debug(`to-frontend-update-keyboard-devices event: identical`);
    }
  }, [latestBackendKeyboardUpdates]);

  useEffect(() => {
    if (JSON.stringify(latestBackendMouseUpdates) !== JSON.stringify(mouseDevices)) {
      // debug(`to-frontend-update-mouse-devices not identical: ${JSON.stringify(latestBackendMouseUpdates)} | ${JSON.stringify(mouseDevices)}`);
      setMouseDevices(latestBackendMouseUpdates);
    } else {
      // debug(`to-frontend-update-mouse-devices event: identical`);
    }
  }, [latestBackendMouseUpdates]);

  const refreshKeyboards = () => {
    debug(`invoke refresh_keyboards`);
    invoke("refresh_keyboards");
  };

  const refreshMouses = () => {
    debug(`invoke refresh_mouses`);
    invoke("refresh_mouses");
  };

  useEffect(() => {
    const unlisten = listen<KeyboardDevice[]>("to-frontend-update-keyboard-devices", (event) => {
      // debug(`to-frontend-update-keyboard-devices event: ${JSON.stringify(event.payload)} ${event.payload.length}`);
      const devices = event.payload;
      setLatestBackendKeyboardUpdates(devices);
    });

    // Refresh does not work well when the code is sync
    new Promise(f => { setTimeout(f, 0); }).then(() => {
      refreshKeyboards();
    });

    return () => {
      unlisten.then(f => f());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<MouseDevice[]>("to-frontend-update-mouse-devices", (event) => {
      // debug(`to-frontend-update-mouse-devices event: ${JSON.stringify(event.payload)} ${event.payload.length}`);
      const devices = event.payload;
      setLatestBackendMouseUpdates(devices);
    });

    // Refresh does not work well when the code is sync
    new Promise(f => { setTimeout(f, 0); }).then(() => {
      refreshMouses();
    });

    return () => {
      unlisten.then(f => f());
    };
  }, []);

  const updateKeyboards = (keyboards: KeyboardDevice[]) => {
    debug(`invoke update_keyboards: ${JSON.stringify(keyboardDevices)}`);
    invoke("update_keyboards", { updated_keyboards: JSON.stringify(keyboards) });
  };

  const updateMouses = (mouses: MouseDevice[]) => {
    debug(`invoke update_mouses: ${JSON.stringify(mouseDevices)}`);
    invoke("update_mouses", { updated_mouses: JSON.stringify(mouses) });
  };

  useEffect(() => {
    if (keyboardDevices.length > 0) {
      updateKeyboards(keyboardDevices);
    }
  }, [keyboardDevices]);

  useEffect(() => {
    if (mouseDevices.length > 0) {
      updateMouses(mouseDevices);
    }
  }, [mouseDevices]);

  /*
    Keyboards and mice differ only in which suffix marks a virtual device and
    which setter to call, so one renderer covers both lists.
  */
  const renderDeviceList = (
    devices: KeyboardDevice[],
    virtualSuffix: string,
    setDevices: React.Dispatch<React.SetStateAction<KeyboardDevice[]>>,
  ) => (
    <div className="device-list">
      {devices.map((device) => {
        const isVirtual = device.name.endsWith(virtualSuffix);
        const isCapturing = device.active && !isVirtual;
        return (
          <Collapsible.Root key={`${device.id}-${device.name}`}>
            <div className="device-row">
              <div className="device-identity">
                <span className={`device-name${isCapturing ? ' device-name-capturing' : ''}`}>
                  {device.name}
                </span>
                {isVirtual && <span className="device-tag">Created by this app</span>}
                <Collapsible.Trigger className="icon-button collapsible-arrow collapsible-arrow-collapsed" aria-label={`Show id of ${device.name}`}>
                  <ChevronDownIcon />
                </Collapsible.Trigger>
                <Collapsible.Trigger className="icon-button collapsible-arrow collapsible-arrow-opened" aria-label={`Hide id of ${device.name}`}>
                  <ChevronUpIcon />
                </Collapsible.Trigger>
              </div>
              {!isVirtual && (
                <Switch.Root
                  className="SwitchRoot"
                  aria-label={`Capture ${device.name}`}
                  checked={device.active}
                  onCheckedChange={() => setDevices((prev) => {
                    const updated = JSON.parse(JSON.stringify(prev)) as KeyboardDevice[];
                    const self = updated.find((prevDevice) => prevDevice.name === device.name && prevDevice.id === device.id);
                    if (self) {
                      self.active = !self.active;
                    }
                    return updated;
                  })}
                >
                  <Switch.Thumb className="SwitchThumb" />
                </Switch.Root>
              )}
            </div>
            <Collapsible.Content className="device-details">
              <span className="mono">{device.id}</span>
            </Collapsible.Content>
          </Collapsible.Root>
        );
      })}
    </div>
  );

  const capturedKeyboards = keyboardDevices.filter((device) => device.active && !device.name.endsWith(VIRTUAL_KEYBOARD_SUFFIX)).length;
  const capturedMouses = mouseDevices.filter((device) => device.active && !device.name.endsWith(VIRTUAL_MOUSE_SUFFIX)).length;

  return (
    <div className="devices">
      <p style={{ marginTop: 'var(--gap-3)' }}>
        Turn on the keyboards and mice whose events this machine should send to other
        machines. Anything left off keeps working normally on this machine only.
      </p>

      <div className="device-section-header">
        <div className="eyebrow">
          Keyboards
          <span className="eyebrow-count">{capturedKeyboards} captured</span>
        </div>
        <button className="icon-button" onClick={() => refreshKeyboards()} aria-label="Rescan keyboards" title="Rescan keyboards">
          <ReloadIcon />
        </button>
      </div>
      {keyboardDevices.length > 0
        ? renderDeviceList(keyboardDevices, VIRTUAL_KEYBOARD_SUFFIX, setKeyboardDevices)
        : <div className="empty-state"><p>No keyboard found. Rescan after plugging one in.</p></div>}

      <div className="device-section-header">
        <div className="eyebrow">
          Mice
          <span className="eyebrow-count">{capturedMouses} captured</span>
        </div>
        <button className="icon-button" onClick={() => refreshMouses()} aria-label="Rescan mice" title="Rescan mice">
          <ReloadIcon />
        </button>
      </div>
      {mouseDevices.length > 0
        ? renderDeviceList(mouseDevices, VIRTUAL_MOUSE_SUFFIX, setMouseDevices)
        : <div className="empty-state"><p>No mouse found. Rescan after plugging one in.</p></div>}
    </div>
  );
}

export default Devices;
