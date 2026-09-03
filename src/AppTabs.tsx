import * as Tabs from "@radix-ui/react-tabs";
import { HomeIcon, KeyboardIcon, FileTextIcon, GearIcon, InfoCircledIcon } from "@radix-ui/react-icons";
import { useContext, useEffect, useState } from "react";
import logo from "/universalkvm.svg";
import Logging from "./pages/Logging";
import Home from "./pages/Home";
import "./AppTabs.css";
import { debug } from "@tauri-apps/plugin-log";
import Settings from "./pages/Settings";
import Devices from "./pages/Devices";
import About from "./pages/About";
import { GlobalContext } from "./App";

function AppTabs() {
  const global = useContext(GlobalContext);
  const [selectedTab, setSelectedTab] = useState("home");

  const [errorLogsCount, setErrorLogsCount] = useState(0);
  useEffect(() => {
    setErrorLogsCount(global.logs.reduce((count, log) => log.level <= 1 ? count + 1 : count, 0));
  }, [global.logs]);

  return (
    <Tabs.Root
      value={selectedTab}
      activationMode="manual"
      onValueChange={(newTab) => {
        debug(newTab);
        setSelectedTab(newTab)}
      }
      className="app-tabs"
    >
      {/* Brand and navigation share one bar, so the monitor layout keeps the height. */}
      <div className="app-bar">
        <div className="brand" data-tauri-drag-region>
          <img src={logo} alt="" className="brand-mark" />
          <span className="brand-name">
            Universal<span className="brand-name-accent">KVM</span>
          </span>
        </div>

        <Tabs.List aria-label="Application tabs" className="tabs-list">
          <Tabs.Trigger value="home" className="tabs-trigger">
            <HomeIcon className="tab-icon" />
            Machines
          </Tabs.Trigger>
          <Tabs.Trigger value="devices" className="tabs-trigger">
            <KeyboardIcon className="tab-icon" />
            Devices
          </Tabs.Trigger>
          <Tabs.Trigger value="settings" className="tabs-trigger">
            <GearIcon className="tab-icon" />
            Settings
          </Tabs.Trigger>
          <Tabs.Trigger value="logging" className="tabs-trigger">
            <FileTextIcon className="tab-icon" />
            Logs
            {errorLogsCount > 0 && <span className="tab-badge">{errorLogsCount}</span>}
          </Tabs.Trigger>
          <Tabs.Trigger value="about" className="tabs-trigger">
            <InfoCircledIcon className="tab-icon" />
            About
          </Tabs.Trigger>
        </Tabs.List>
      </div>

      <Tabs.Content value="home" className="tab">
        <Home />
      </Tabs.Content>
      <Tabs.Content value="devices" className="tab">
        <Devices />
      </Tabs.Content>
      <Tabs.Content value="logging" className="tab">
        <Logging />
      </Tabs.Content>
      <Tabs.Content value="settings" className="tab">
        <Settings />
      </Tabs.Content>
      <Tabs.Content value="about" className="tab">
        <About />
      </Tabs.Content>
    </Tabs.Root>
  );
}

export default AppTabs;
