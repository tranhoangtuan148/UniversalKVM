import "./Monitors.css";
import { AppMonitors, BorderPair } from "../interfaces/global";
import MonitorsViewer from "./MonitorsViewer";
import { useEffect, useState } from "react";
import { CheckIcon, Cross2Icon, Pencil2Icon } from "@radix-ui/react-icons";
import { debug } from "@tauri-apps/plugin-log";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  appsMonitors: AppMonitors[],
  borders: BorderPair[],
  focusedId: string,
}
const Monitors: React.FC<Props> = (props) => {
  const { appsMonitors, borders, focusedId } = props;

  const [isEditMode, setIsEditMode] = useState<boolean>(false);


  const submitEditMonitors = () => {
    debug(`invoke submit_edit_monitors`);
    let appsMonitors = renderedAppsMonitors.map((app) => ({
      id: app.id,
      set_of_monitors: {
        offset_x: Math.round(app.offset_x),
        offset_y: Math.round(app.offset_y),
        monitors: app.monitors,
      },
    }));
    let borders: BorderPair[] = renderedBorders.map((border) => ({
      color_r: border.color_r,
      color_g: border.color_g,
      color_b: border.color_b,
      pair: border.pair.map((border) => ({
        start: Math.round(border.start),
        end: Math.round(border.end),
        border: border.border,
        monitor_index: border.monitor_index,
        monitors_id: border.monitors_id,
        app_id: border.app_id,
      })),
    }));
    invoke("submit_edit_monitors", { edit: JSON.stringify({ apps: appsMonitors, borders }) });
    setIsEditMode(false);
  };

  const REASONABLE_LIMIT = 10000000;

  function getMinX(app: AppMonitors): number {
    if (app.monitors.length === 0) {
      return app.offset_x;
    }
    let minX = REASONABLE_LIMIT;
    for (const monitor of app.monitors) {
      minX = Math.min(app.offset_x + monitor.x, minX);
    }
    return minX;
  }
  function getMaxX(app: AppMonitors): number {
    if (app.monitors.length === 0) {
      return app.offset_x;
    }
    let maxX = -REASONABLE_LIMIT;
    for (const monitor of app.monitors) {
      maxX = Math.max(app.offset_x + monitor.x + monitor.width, maxX);
    }
    return maxX;
  }
  function getMinY(app: AppMonitors): number {
    if (app.monitors.length === 0) {
      return app.offset_y;
    }
    let minY = REASONABLE_LIMIT;
    for (const monitor of app.monitors) {
      minY = Math.min(app.offset_y + monitor.y, minY);
    }
    return minY;
  }
  function getMaxY(app: AppMonitors): number {
    if (app.monitors.length === 0) {
      return app.offset_y;
    }
    let maxY = -REASONABLE_LIMIT;
    for (const monitor of app.monitors) {
      maxY = Math.max(app.offset_y + monitor.y + monitor.height, maxY);
    }
    return maxY;
  }

  let copiedAppsMonitors: AppMonitors[] = appsMonitors.map((app) => {
    return {
      name: app.name,
      id: app.id,
      monitors: app.monitors.map((monitor) => ({
        x: monitor.x,
        y: monitor.y,
        width: monitor.width,
        height: monitor.height,
        color_r: monitor.color_r,
        color_g: monitor.color_g,
        color_b: monitor.color_b,
      })),
      offset_x: app.offset_x,
      offset_y: app.offset_y,
    };
  });

  // Knowing total width and height of all apps
  let allAppsWidth = 0;
  let allAppsHeight = 0;

  // Global view boundaries
  let minX = REASONABLE_LIMIT;
  let maxX = -REASONABLE_LIMIT;
  let minY = REASONABLE_LIMIT;
  let maxY = -REASONABLE_LIMIT;
  for (let appMonitors of copiedAppsMonitors) {
    allAppsWidth += getMaxX(appMonitors) - getMinX(appMonitors);
    allAppsHeight += getMaxY(appMonitors) - getMinY(appMonitors);

    minX = Math.min(minX, getMinX(appMonitors));
    maxX = Math.max(maxX, getMaxX(appMonitors));
    minY = Math.min(minY, getMinY(appMonitors));
    maxY = Math.max(maxY, getMaxY(appMonitors));
  }

  let svgMinX = minX;
  let svgMinY = minY;
  let svgWidth = maxX - minX;
  let svgHeight = maxY - minY;

  // In edit mode, stretch the svg area to allow some space around the monitors
  let svgMarginX = 0.5 * (allAppsWidth - (maxX - minX) / 2);
  let svgMarginY = 0.5 * (allAppsHeight - (maxY - minY) / 2);
  if (isEditMode) {
    svgMinX -= svgMarginX;
    svgMinY -= svgMarginY;
    svgWidth += 2 * svgMarginX;
    svgHeight += 2 * svgMarginY;
  }

  const [renderedAppsMonitors, setRenderedAppsMonitors] = useState<AppMonitors[]>([]);
  useEffect(() => {
    if (renderedAppsMonitors.length === 0) {
      setRenderedAppsMonitors(copiedAppsMonitors);
    } else if (!isEditMode) {
      setRenderedAppsMonitors(copiedAppsMonitors);
    }
    // When editing, monitors should not be updated, to prevent canceling user changes.
  }, [appsMonitors, isEditMode]);

  let copiedBorders: BorderPair[] = borders.map((border) => ({
    pair: border.pair.map((border) => ({
      start: border.start,
      end: border.end,
      border: border.border,
      monitor_index: border.monitor_index,
      monitors_id: border.monitors_id,
      app_id: border.app_id,
    })),
    color_r: border.color_r,
    color_g: border.color_g,
    color_b: border.color_b,
  }))
  const [renderedBorders, setRenderedBorders] = useState<BorderPair[]>([]);
  useEffect(() => {
    if (renderedAppsMonitors.length === 0) {
      setRenderedBorders(copiedBorders);
    } else if (!isEditMode) {
      setRenderedBorders(copiedBorders);
    }
    // When editing, borders should not be updated, to prevent canceling user changes.
  }, [appsMonitors, isEditMode]);

  const canEdit = renderedAppsMonitors.length > 0 && renderedAppsMonitors[0].monitors.length > 0;
  const monitorCount = renderedAppsMonitors.reduce((total, app) => total + app.monitors.length, 0);

  return (
    <section className="monitors">
      <div className="eyebrow">
        Monitor layout
        <span className="eyebrow-count">
          {monitorCount} {monitorCount === 1 ? 'screen' : 'screens'} · {renderedBorders.length} {renderedBorders.length === 1 ? 'crossing' : 'crossings'}
        </span>
      </div>

      <div className={`monitors-frame${isEditMode ? ' monitors-frame-editing' : ''}`}>
        <div className="monitors-toolbar">
          <span className="monitors-toolbar-label">
            {isEditMode ? 'Editing layout' : 'Arrangement across machines'}
          </span>
          <div className="monitors-toolbar-actions">
            {isEditMode ? (
              <>
                <button onClick={() => setIsEditMode(false)}>
                  <Cross2Icon />
                  Discard
                </button>
                <button className="primary" onClick={() => submitEditMonitors()}>
                  <CheckIcon />
                  Save layout
                </button>
              </>
            ) : canEdit && (
              <button onClick={() => setIsEditMode(true)}>
                <Pencil2Icon />
                Edit layout
              </button>
            )}
          </div>
        </div>

        {isEditMode && (
          <p className="monitors-hint">
            Drag a machine to reposition it. To let the cursor cross between two machines,
            click an edge on one screen, then click the facing edge on the other.
          </p>
        )}

        <div className="monitors-field">
          <MonitorsViewer
            appsMonitors={renderedAppsMonitors}
            borders={renderedBorders}
            focusedId={focusedId}
            isEditMode={isEditMode}
            svgMinX={svgMinX}
            svgMinY={svgMinY}
            svgWidth={svgWidth}
            svgHeight={svgHeight}
          />
        </div>
      </div>
    </section>
  );
};

export default Monitors;
