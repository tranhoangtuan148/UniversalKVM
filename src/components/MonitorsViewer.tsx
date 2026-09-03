import { useState } from "react";
import { TrashIcon } from "@radix-ui/react-icons";
import "./Warn.css";
import { AppMonitors, BorderPair, MonitorBorder, Monitor } from "../interfaces/global";
import "./MonitorsViewer.css";
import { invoke } from "@tauri-apps/api/core";

enum EditMode {
  Translation,
  AddBorders,
  SelectBorder,
}

enum Border {
  Left = 0,
  Right = 1,
  Top = 2,
  Bottom = 3,
}

interface Props {
  appsMonitors: AppMonitors[],
  borders: BorderPair[],
  focusedId: string,

  isEditMode: boolean,

  // Determines the viewport
  svgMinX: number,
  svgMinY: number,
  svgWidth: number,
  svgHeight: number,
}
const MonitorsViewer: React.FC<Props> = (props) => {
  const { appsMonitors, borders, focusedId, isEditMode, svgMinX, svgMinY, svgWidth, svgHeight } = props;

  const maxMonitorWidth = Math.max(...appsMonitors.map((appMonitors) =>
    appMonitors.monitors.reduce((maximum, monitor) => Math.max(monitor.width, maximum), 1)
  ));
  const maxMonitorHeight = Math.max(...appsMonitors.map((appMonitors) =>
    appMonitors.monitors.reduce((maximum, monitor) => Math.max(monitor.height, maximum), 1)
  ));

  const editedAppsMonitors = appsMonitors;

  const setFocusedId = (focused_id: string, x: number, y: number) => {
    invoke("set_focused_id", { focus: JSON.stringify({ focused_id, x: Math.round(x), y: Math.round(y) }) });
  };

  const getFocusClassName = (focus: boolean): string => {
    return focus ? 'monitor-with-focus' : '';
  };

  function getMonitorInfo(border: MonitorBorder, appsMonitors: AppMonitors[]): { offsetX: number, offsetY: number, monitor: Monitor | null } {
    for (const appMonitors of appsMonitors) {
      if (border.app_id === appMonitors.id) {
        return { offsetX: appMonitors.offset_x, offsetY: appMonitors.offset_y, monitor: appMonitors.monitors[border.monitor_index] };
      }
    }
    return { offsetX: 0, offsetY: 0, monitor: null };
  }

  const REASONABLE_LIMIT = 10000000;
  function getRelativeMiddleX(app: AppMonitors | undefined): number {
    if (app) {
      if (app.monitors.length === 0) {
        return app.offset_x;
      }
      let maxX = -REASONABLE_LIMIT;
      let minX = REASONABLE_LIMIT;
      for (const monitor of app.monitors) {
        maxX = Math.max(monitor.x + monitor.width, maxX);
        minX = Math.min(monitor.x, minX);
      }
      return (maxX + minX) / 2;
    }
    return 0;
  }
  function getRelativeMiddleY(app: AppMonitors | undefined): number {
    if (app) {
      if (app.monitors.length === 0) {
        return app.offset_y;
      }
      let maxY = -REASONABLE_LIMIT;
      let minY = REASONABLE_LIMIT;
      for (const monitor of app.monitors) {
        maxY = Math.max(monitor.y + monitor.height, maxY);
        minY = Math.min(monitor.y, minY);
      }
      return (maxY + minY) / 2;
    }
    return 0;
  }

  function getRandomColor(): { color_r: number, color_g: number, color_b: number } {
    let rgb = [0, 0, 0];
    let pickIndexes = [0, 1, 2];
    let mainColor = Math.floor(Math.random() * 3);
    rgb[mainColor] = 255;
    pickIndexes.splice(mainColor, 1);

    let secondaryColor = Math.floor(Math.random() * 2);
    rgb[pickIndexes[secondaryColor]] = Math.floor(Math.random() * 256); // Generate a secondary color of intensity between 0 to 255

    return { color_r: rgb[0], color_g: rgb[1], color_b: rgb[2] };
  }

  function colorToHex(color: { color_r?: number, color_g?: number, color_b?: number}): string {
    const red = color?.color_r ?? 0;
    const green = color?.color_g ?? 0;
    const blue = color?.color_b ?? 0;
    return `#${red.toString(16).padStart(2, '0')}${green.toString(16).padStart(2, '0')}${blue.toString(16).padStart(2, '0')}`
  }


  function getLineWidth(width: number, height: number): number {
    return 0.125 * Math.min(width, height);
  }

  // Tries to make the name appear as large as possible, but be within the rectangle
  function getSvgFontSize(name: string, width: number, height?: number): number {
    let len = Math.max(name.length + 1);
    let factor = 2 - (1 / len);
    // Using a factor to make the text almost as large as the width
    return factor * Math.min(width - getLineWidth(width, height ?? width), height ?? width) / len
  }

  function getSvgFontDX(name: string, width: number, height: number): number {
    return Math.max(getSvgFontSize(name, width, height) / 4, getLineWidth(width, height));
  }

  // Tries to make the name appear as large as possible, but be within the rectangle
  function getSvgFontDY(name: string, width: number, height: number): number {
    return getSvgFontSize(name, width, height) + getLineWidth(width, height);
  }

  function displayLinkedBorder(
    monitorInfo: { offsetX: number, offsetY: number, monitor: Monitor | null },
    monitorBorder: MonitorBorder | undefined,
    params?: {
      noHitBox?: boolean,
      color?: string,
      opacity?: number,
      pattern?: string,
    }
  ) {
    if (!monitorBorder || !monitorInfo.monitor) {
      return null;
    }
    const monitor = monitorInfo.monitor;

    let x1 = monitor.x + monitorInfo.offsetX;
    let y1 = monitor.y + monitorInfo.offsetY;
    let x2 = x1;
    let y2 = y1;
    let x3 = x1;
    let y3 = y1;
    let x4 = x1;
    let y4 = y1;
    let thickness = Math.min(getLineWidth(monitor.width, monitor.height), (monitorBorder.end - monitorBorder.start) / 2);
    if (monitorBorder.border === Border.Left) {
      y1 += monitorBorder.start;
      x2 += thickness;
      y2 += monitorBorder.start + thickness;
      x3 += thickness;
      y3 += monitorBorder.end - thickness;
      y4 += monitorBorder.end;
    } else if (monitorBorder.border === Border.Right) {
      x1 += monitor.width;
      y1 += monitorBorder.start;
      x2 += monitor.width - thickness;
      y2 += monitorBorder.start + thickness;
      x3 += monitor.width - thickness;
      y3 += monitorBorder.end - thickness;
      x4 += monitor.width;
      y4 += monitorBorder.end;
    } else if (monitorBorder.border === Border.Top) {
      x1 += monitorBorder.start;
      x2 += monitorBorder.start + thickness;
      y2 += thickness;
      x3 += monitorBorder.end - thickness;
      y3 += thickness;
      x4 += monitorBorder.end;
    } else if (monitorBorder.border === Border.Bottom) {
      x1 += monitorBorder.start;
      y1 += monitor.height;
      x2 += monitorBorder.start + thickness;
      y2 += monitor.height - thickness;
      x3 += monitorBorder.end - thickness;
      y3 += monitor.height - thickness;
      x4 += monitorBorder.end;
      y4 += monitor.height;
    }
    // Draw a trapeze or a triangle (corners have a small diagonal line)
    return <polygon
      points={`${x1},${y1} ${x2},${y2} ${x3},${y3} ${x4},${y4}`}
      fill={params?.pattern ? `url(#${params.pattern})` : (params?.color)}
      opacity={`${params?.opacity ?? 1}`}
      pointerEvents={params?.noHitBox ? 'none' : 'auto'}
    />
  }

  const [editMode, setEditMode] = useState<EditMode | undefined>(undefined);
  const [selectedApp, setSelectedApp] = useState<string>('');

   const [hoveredOnMonitor, setHoveredOnMonitor] = useState<string | undefined>(undefined);
  const [hoveredBorder, setHoveredBorder] = useState<{ app: AppMonitors, monitorIndex: number, border: number } | undefined>(undefined);

  function isOppositeBorder(border1: number, border2: number | undefined): boolean {
    return (border1 === Border.Left && border2 === Border.Right)
      || (border1 === Border.Right && border2 === Border.Left)
      || (border1 === Border.Top && border2 === Border.Bottom)
      || (border1 === Border.Bottom && border2 === Border.Top);
  }

  // Two borders has to be selected to create a logical border between two monitors
  const [selectedAppBorder1, setSelectedAppBorder1] = useState<{app: AppMonitors, monitorIndex: number, border: MonitorBorder} | undefined>(undefined);
  const [nextBorderColor, setNextBorderColor] = useState<{ color_r: number, color_g: number, color_b: number }>(getRandomColor());

  const [selectedBorderPair, setSelectedBorderPair] = useState<number | undefined>(undefined);

  function setSelectedAppBorder(app: AppMonitors, monitorIndex: number, monitorBorder: MonitorBorder) {
    if (!selectedAppBorder1) {
      setEditMode(EditMode.AddBorders);
      setSelectedAppBorder1({
        app,
        monitorIndex,
        border: monitorBorder
      });
    } else {
      // Add a pair of borders
      let borderPair: BorderPair = {
        color_r: nextBorderColor.color_r,
        color_g: nextBorderColor.color_g,
        color_b: nextBorderColor.color_b,
        pair: [selectedAppBorder1.border, monitorBorder],
      };
      borders.push(borderPair);
      setNextBorderColor(getRandomColor());

      // Reset border selection
      setEditMode(undefined);
      setSelectedAppBorder1(undefined);
    }
  }

  // When a first border has been selected, only an opposite border can be selected from another set of monitors
  function isHoveredBorderValid(selectedApp: {app: AppMonitors, monitorIndex: number, border: MonitorBorder} | undefined, app: AppMonitors, border: number): boolean {
    if (!selectedAppBorder1) {
      return true;
    }

    if (selectedApp?.app?.id === app.id) {
      return false;
    }

    return isOppositeBorder(border, selectedApp?.border?.border);
  }

  // Cursor translated into svg coordinates
  const [svgCursorX, setSvgCursorX] = useState<number>(0);
  const [svgCursorY, setSvgCursorY] = useState<number>(0);

  function isOverlapping(start1: number, end1: number, start2: number, end2: number): boolean {
    return (start1 <= start2 && start2 <= end1)
      || (start1 <= end2 && end2 <= end1)
      || (start2 <= start1 && end1 <= end2);
  }

  // Assumes that there is an overlap
  function getOverlap(start1: number, end1: number, start2: number, end2: number): { start: number, end: number } {
    return {
      start: Math.max(start1, start2),
      end: Math.min(end1, end2),
    };
  }

  // With multiple monitors, the interior borders should not be valid border choice.
  function getInvalidBorders(currentMonitor: Monitor, monitors: Monitor[], border: number): MonitorBorder[] {
    const result = [];
    for (const monitor of monitors) {
      // Ignore the same monitor as currentMonitor
      if (monitor.x === currentMonitor.x && monitor.y === currentMonitor.y
        && monitor.width === currentMonitor.width && monitor.height === currentMonitor.height
      ) {
        continue;
      }

      if (border === Border.Left) {
        const relOffset = monitor.y - currentMonitor.y;
        if (monitor.x + monitor.width <= currentMonitor.x && isOverlapping(relOffset, relOffset + monitor.height, 0, currentMonitor.height)) {
          const overlap = getOverlap(relOffset, relOffset + monitor.height, 0, currentMonitor.height);
          result.push({
            start: overlap.start,
            end: overlap.end,
            border,
            color_r: 0,
            color_g: 0,
            color_b: 0,
            monitor_index: 0,
            monitors_id: '',
            app_id: '',
          });
        }
      }
      if (border === Border.Right) {
        const relOffset = monitor.y - currentMonitor.y;
        if ( currentMonitor.x + currentMonitor.width <= monitor.x && isOverlapping(relOffset, relOffset + monitor.height, 0, currentMonitor.height)) {
          const overlap = getOverlap(relOffset, relOffset + monitor.height, 0, currentMonitor.height);
          result.push({
            start: overlap.start,
            end: overlap.end,
            border,
            color_r: 0,
            color_g: 0,
            color_b: 0,
            monitor_index: 0,
            monitors_id: '',
            app_id: '',
          });
        }
      }
      if (border === Border.Top) {
        const relOffset = monitor.x - currentMonitor.x;
        if (monitor.y + monitor.height <= currentMonitor.y && isOverlapping(relOffset, relOffset + monitor.width, 0, currentMonitor.width)) {
          const overlap = getOverlap(relOffset, relOffset + monitor.width, 0, currentMonitor.width);
          result.push({
            start: overlap.start,
            end: overlap.end,
            border,
            color_r: 0,
            color_g: 0,
            color_b: 0,
            monitor_index: 0,
            monitors_id: '',
            app_id: '',
          });
        }
      }
      if (border === Border.Bottom) {
        const relOffset = monitor.x - currentMonitor.x;
        if (currentMonitor.y + currentMonitor.height <= monitor.y && isOverlapping(relOffset, relOffset + monitor.width, 0, currentMonitor.width)) {
          const overlap = getOverlap(relOffset, relOffset + monitor.width, 0, currentMonitor.width);
          result.push({
            start: overlap.start,
            end: overlap.end,
            border,
            color_r: 0,
            color_g: 0,
            color_b: 0,
            monitor_index: 0,
            monitors_id: '',
            app_id: '',
          });
        }
      }
    }

    return result;
  }

  function isOverlappingWithExistingBorder(start: number, end: number, border: number): boolean {
    for (const existingBorder of borders) {
      if (existingBorder.pair[0].border === border && isOverlapping(start, end, existingBorder.pair[0].start, existingBorder.pair[0].end)) {
        return true;
      }
      if (existingBorder.pair[1].border === border && isOverlapping(start, end, existingBorder.pair[1].start, existingBorder.pair[1].end)) {
        return true;
      }
    }

    return false;
  }

  // A border that is tiny is not worth to display as an option; it would also be hard to remove
  const MIN_PIXELS = 32;

  // Returns the nearest available border, if any
  function getAvailableBorder(appMonitors: { app: AppMonitors, monitorIndex: number, border: number }, cursor: number): { start: number, end: number} | undefined {
    let startMax = 0;
    const monitor = appMonitors.app.monitors[appMonitors.monitorIndex];
    const border = appMonitors.border;
    let endMin = appMonitors.border === Border.Left || appMonitors.border === Border.Right ? monitor.height : monitor.width;

    let monitorBorders = [];
    for (const border of borders) {
      if (border.pair[0].app_id === appMonitors.app.id && border.pair[0].monitor_index === appMonitors.monitorIndex) {
        monitorBorders.push(border.pair[0]);
      } else if (border.pair[1].app_id === appMonitors.app.id && border.pair[1].monitor_index === appMonitors.monitorIndex) {
        monitorBorders.push(border.pair[1]);
      }
    }
    const unavailableBorders = monitorBorders.concat(getInvalidBorders(monitor, appMonitors.app.monitors, border));
    for (const existingBorder of unavailableBorders) {
      if (existingBorder.border !== border) {
        continue;
      }

      if (isOverlappingWithExistingBorder(cursor, cursor, border)) {
        return undefined;
      }

      if (existingBorder.end <= cursor && existingBorder.end > startMax) {
        startMax = existingBorder.end;
      }
      if (existingBorder.start >= cursor && existingBorder.start < endMin) {
        endMin = existingBorder.start;
      }
    }

    for (const invalidBorder of getInvalidBorders(monitor, appMonitors.app.monitors, border)) {
      if (isOverlapping(invalidBorder.start, invalidBorder.end, cursor, cursor)) {
        return undefined;
      }
    }

    if (endMin - startMax >= MIN_PIXELS) {
      return { start: Math.round(startMax), end: Math.round(endMin) };  // start and end are integers in Rust
    }
    return undefined;
  }

  // The returned value is either the longest available segment of the border, or
  // a segment matching another monitor border
  function getSuggestedBorder(x: number, y: number, hoveredBorder: { app: AppMonitors, monitorIndex: number, border: number }, apps: AppMonitors[]): MonitorBorder | undefined {
    const monitor = hoveredBorder.app.monitors[hoveredBorder.monitorIndex];
    let start = hoveredBorder.border === Border.Left || hoveredBorder.border === Border.Right
      ? 0
      : 0;
    let startAbsolute = hoveredBorder.border === Border.Left || hoveredBorder.border === Border.Right
      ? start + hoveredBorder.app.offset_y + monitor.y
      : start + hoveredBorder.app.offset_x + monitor.x;
    let end = hoveredBorder.border === Border.Left || hoveredBorder.border === Border.Right
      ? monitor.height
      : monitor.width;
    let endAbsolute = hoveredBorder.border === Border.Left || hoveredBorder.border === Border.Right
      ? end + hoveredBorder.app.offset_y + monitor.y
      : end + hoveredBorder.app.offset_x + monitor.x;
    const cursorAbsolute = hoveredBorder.border === Border.Left || hoveredBorder.border === Border.Right
      ? y
      : x;
    const cursorRelative = cursorAbsolute - startAbsolute;
    let cursorDistanceToBorder = 0;
    if (hoveredBorder.border === Border.Left) {
      cursorDistanceToBorder = Math.abs(x - (monitor.x + hoveredBorder.app.offset_x));
    } else if (hoveredBorder.border === Border.Right) {
      cursorDistanceToBorder = Math.abs(x - (monitor.x + hoveredBorder.app.offset_x + monitor.width));
    } else if (hoveredBorder.border === Border.Top) {
      cursorDistanceToBorder = Math.abs(y - (monitor.y + hoveredBorder.app.offset_y));
    } else {
      cursorDistanceToBorder = Math.abs(y - (monitor.y + hoveredBorder.app.offset_y + monitor.height));
    }

    let otherBorders: { start: number, end: number , distance: number }[] = [];
    for (const app of apps) {
      if (app.id == hoveredBorder.app.id) {
        continue;
      }

      for (const otherMonitor of app.monitors) {
        let monitorDistance = 0;
        let otherStartAbsolute = 0;
        let otherEndAbsolute = 0;
        if (hoveredBorder.border === Border.Left || hoveredBorder.border === Border.Right) {
          monitorDistance = hoveredBorder.border === Border.Left
            ? Math.abs((hoveredBorder.app.offset_x + monitor.x) - ((app.offset_x + otherMonitor.x) + otherMonitor.width))
            : Math.abs((hoveredBorder.app.offset_x + monitor.x) + monitor.width - (app.offset_x + otherMonitor.x));
          otherStartAbsolute = app.offset_y + otherMonitor.y;
          otherEndAbsolute = (app.offset_y + otherMonitor.y) + otherMonitor.height;
        } else {
          monitorDistance = hoveredBorder.border === Border.Top
            ? Math.abs((hoveredBorder.app.offset_y + monitor.y) - ((app.offset_y + otherMonitor.y) + otherMonitor.height))
            : Math.abs((hoveredBorder.app.offset_y + monitor.y) + monitor.height - (app.offset_y + otherMonitor.y));
          otherStartAbsolute = app.offset_x + otherMonitor.x;
          otherEndAbsolute = (app.offset_x + otherMonitor.x) + otherMonitor.width;
        }

        if ((startAbsolute < otherStartAbsolute && otherStartAbsolute < endAbsolute)
          || (startAbsolute < otherEndAbsolute && otherEndAbsolute < endAbsolute)
          || (otherStartAbsolute < startAbsolute && endAbsolute < otherEndAbsolute)
        ) {
          let start = 0;
          let end = 0;
          if (hoveredBorder.border === Border.Left || hoveredBorder.border === Border.Right) {
            start = Math.max(otherStartAbsolute - startAbsolute, 0);
            end = Math.min(otherEndAbsolute + monitor.height - endAbsolute, monitor.height);
          } else {
            start = Math.max(otherStartAbsolute - startAbsolute, 0);
            end = Math.min(otherEndAbsolute + monitor.width - endAbsolute, monitor.width);
          }

          const availableBorder = getAvailableBorder(hoveredBorder, (start + end) / 2);
          if (availableBorder) {
            otherBorders.push({ start: Math.max(start, availableBorder.start), end: Math.min(end, availableBorder.end), distance: monitorDistance });
          }
        } else {
          // Ignore monitors that are not aligned
          continue;
        }
        
      }
    }
    otherBorders = otherBorders.sort((a, b) => Math.abs(a.distance) - Math.abs(b.distance));

    // Magnetism is used here. the nearer the cursor is to center of a segment, the more likely it is to be selected.
    let bestMatch = 50; /// default choice has a score of 50, and the maximum value is 100
    let furthestDistance = otherBorders.length > 0 ? otherBorders[otherBorders.length - 1].distance : 1;
    for (const otherBorder of otherBorders) {
      const otherLength = otherBorder.end - otherBorder.start;
      const otherMiddle = otherBorder.start + otherLength / 2;
      const hitBoxWidth = getLineWidth(monitor.width, monitor.height);
      const score = (30 * ((hitBoxWidth - cursorDistanceToBorder) / hitBoxWidth) * (furthestDistance - otherBorder.distance) / furthestDistance)
        + (70 * (otherLength - Math.abs(otherMiddle - cursorRelative)) / otherLength);
      if (score > bestMatch) {
        start = otherBorder.start;
        end = otherBorder.end;
        bestMatch = score;
      }
    }

    // If the default choice is picked, choose the nearest segment to the cursor
    if (bestMatch === 50) {
      const availableBorder = getAvailableBorder(hoveredBorder, cursorRelative);
      if (availableBorder) {
        return {
          start: Math.round(availableBorder.start), // Is an integer in Rust
          end: Math.round(availableBorder.end), // Is an integer in Rust
          border: hoveredBorder.border,
          monitor_index: hoveredBorder.monitorIndex,
          monitors_id: '', // Unused on the frontend
          app_id: hoveredBorder.app.id,
        };
      } else {
        return undefined;
      }
    } else {
      return {
        start: Math.round(start), // Is an integer in Rust
        end: Math.round(end), // Is an integer in Rust
        border: hoveredBorder.border,
        monitor_index: hoveredBorder.monitorIndex,
        monitors_id: '', // Unused on the frontend
        app_id: hoveredBorder.app.id,
      };
    }
  }

  return (
    <>
      <div
        style={{ lineHeight: 0 }}
        id="svg-monitors-area"
        onMouseMove={(event) => {
          const container = document.getElementById("svg-monitors-area");
          const bounds = container?.getBoundingClientRect();
          let relX = (event?.clientX ?? 0) - (bounds?.left ?? 0);
          let relY = (event?.clientY ?? 0) - (bounds?.top ?? 0);

          let svgX = svgMinX + relX * svgWidth / (bounds?.width ?? svgWidth);
          let svgY = svgMinY + relY * svgHeight / (bounds?.height ?? svgHeight);
          setSvgCursorX(svgX);
          setSvgCursorY(svgY);
        }}
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox={`${svgMinX} ${svgMinY} ${svgWidth} ${svgHeight}`}
          onClick={() => {
            // If a click is not on a monitor, cancel user actions
            setEditMode(undefined);
            setSelectedApp(''); // Cancel set of monitors translation
            setSelectedAppBorder1(undefined); // Cancel border selection
            setSelectedBorderPair(undefined); // Cancel existing border selection
          }}
          onMouseEnter={() => setHoveredBorder(undefined) /* Unselect hovered border */ }
          onMouseLeave={() => setHoveredBorder(undefined) /* Unselect hovered border */ }
        >
          <defs>
            <pattern id="hatch" patternTransform="rotate(45)" patternUnits="userSpaceOnUse" viewBox="0,0,10,10" width={getLineWidth(maxMonitorWidth, maxMonitorHeight) / 4} height={getLineWidth(maxMonitorWidth, maxMonitorHeight) / 4}>
              <line
                x1={`${0}`} y1={`${0}`}
                x2={`${0}`} y2={`${10}`}
                stroke="white"
                strokeWidth={2}
                opacity={`${1}`}
              />
              <line
                x1={`${5}`} y1={`${0}`}
                x2={`${5}`} y2={`${10}`}
                stroke="black"
                strokeWidth={2}
                opacity={`${1}`}
              />
            </pattern>
          </defs>
          {editedAppsMonitors.map((appMonitors) => (
            <g
              transform={`translate(${appMonitors.offset_x} ${appMonitors.offset_y})`}
              opacity={`${selectedApp === appMonitors.id ? 0.5 : 1.0}`}
              onClick={(event) => {
                if (isEditMode && !editMode) {
                  setEditMode(EditMode.Translation);
                  setSelectedApp(appMonitors.id);
                  event.stopPropagation(); /* To prevent click from unselecting set of monitors */
                }
              }}
            >
              {appMonitors.monitors.map((monitor, index) => (
                <g>
                  <rect
                    width={`${monitor.width}`}
                    height={`${monitor.height}`}
                    x={`${monitor.x}`} y={`${monitor.y}`}
                    rx="2" ry="2"
                    /* A translucent fill plus a solid outline keeps superimposed monitors readable. */
                    fill={colorToHex(monitor)}
                    fillOpacity="0.24"
                    stroke={colorToHex(monitor)}
                    strokeWidth="1.5"
                    vectorEffect="non-scaling-stroke"
                    className={`monitor-plate${!isEditMode && hoveredOnMonitor === appMonitors.id ? ' monitor-hovered' : ''} ${!isEditMode ? getFocusClassName(focusedId === appMonitors.id) : ''}`}
                    onClick={() => {
                      if (!isEditMode) {
                        setFocusedId(appMonitors.id, svgCursorX - appMonitors.offset_x, svgCursorY - appMonitors.offset_y);
                      }
                    }}
                    onMouseEnter={(_) => {
                      setHoveredOnMonitor(appMonitors.id);
                    }}
                    onMouseLeave={(_) => {
                      setHoveredOnMonitor(undefined);
                    }}
                  />
                  {isEditMode && [Border.Bottom, Border.Top, Border.Right, Border.Left].map((border) => (
                    <g
                      onClick={(event) => {
                        if (isEditMode && (!editMode || editMode === EditMode.AddBorders)) {
                          if (hoveredBorder && isHoveredBorderValid(selectedAppBorder1, appMonitors, hoveredBorder.border)) {
                            const monitorBorder = getSuggestedBorder(svgCursorX, svgCursorY, hoveredBorder, editedAppsMonitors);
                            if (monitorBorder) {
                              setSelectedAppBorder(appMonitors, index, monitorBorder);
                            }
                          }

                          event.stopPropagation(); /* To prevent click from translating set of monitors */
                        }
                      }}
                      onMouseEnter={(event) => {
                        if (isEditMode) {
                          setHoveredBorder({ app: appMonitors, monitorIndex: index, border });
                          event.stopPropagation(); /* To prevent multiple hovering borders from being selected */
                        }
                      }}
                      onMouseLeave={(_) => {
                        if (isEditMode) {
                          setHoveredBorder(undefined);
                        }
                      }}
                    >
                      {displayLinkedBorder({ offsetX:0, offsetY:0, monitor }, {
                        start: 0,
                        end: border === Border.Left || border === Border.Right ? monitor.height : monitor.width,
                        border,
                        monitor_index: 0,
                        monitors_id: '',
                        app_id: '',
                      }, { opacity: 0 /* This border is used for mouse detection only */})}
                    </g>
                  ))}
                  {hoveredBorder && hoveredBorder.app.id === appMonitors.id && hoveredBorder.monitorIndex === index
                    && isHoveredBorderValid(selectedAppBorder1, appMonitors, hoveredBorder.border)
                    && (!editMode || editMode === EditMode.AddBorders) && (
                    <g>
                      {displayLinkedBorder(
                        { offsetX:0, offsetY:0, monitor },
                        getSuggestedBorder(svgCursorX, svgCursorY, hoveredBorder, editedAppsMonitors),
                        { noHitBox: true, color: colorToHex(nextBorderColor) }
                      )}
                    </g>
                  )}
                  {selectedAppBorder1 && selectedAppBorder1.app.id === appMonitors.id && selectedAppBorder1.monitorIndex === index && (
                    <g
                      onClick={(event) => {
                        if (isEditMode && editMode === EditMode.AddBorders) {
                          setSelectedAppBorder1(undefined); // Unselect when clicking on the selection
                          event.stopPropagation(); /* To prevent click from translating set of monitors */
                        }
                      }}
                    >
                      {displayLinkedBorder(
                        { offsetX:0, offsetY:0, monitor },
                        selectedAppBorder1.border,
                        { color: colorToHex(nextBorderColor) }
                      )}
                      {displayLinkedBorder(
                        { offsetX:0, offsetY:0, monitor },
                        selectedAppBorder1.border,
                        { pattern: 'hatch' }
                      )}
                    </g>
                  )}
                  <text
                    x={`${monitor.x + getSvgFontDX(appMonitors.name, monitor.width, monitor.height)}`}
                    y={`${monitor.y + getSvgFontDY(appMonitors.name, monitor.width, monitor.height)}`}
                    className="monitor-label"
                    fontSize={getSvgFontSize(appMonitors.name, monitor.width)}
                    style={{ pointerEvents: 'none' }}
                  >
                    {appMonitors.name}
                  </text>
                  <text
                    x={`${monitor.x + getSvgFontDX(`${monitor.width}x${monitor.height}`, monitor.width, monitor.height)}`}
                    y={`${monitor.y + getSvgFontDY(appMonitors.name, monitor.width, monitor.height) + getSvgFontSize(appMonitors.name, monitor.width, monitor.height) / 2 + getSvgFontSize(`${monitor.width}x${monitor.height}`, monitor.width, monitor.height)}`}
                    className="monitor-spec"
                    fontSize={getSvgFontSize(`${monitor.width}x${monitor.height}`, monitor.width, monitor.height)}
                    style={{ pointerEvents: 'none' }}
                  >
                    {`${monitor.width}x${monitor.height}`}
                  </text>
                </g>
              ))}
            </g>
          ))}

          {isEditMode && borders.map((pairedBorder, index) => (
            pairedBorder.pair.map((border) => (
              <g
                onClick={(event) => {
                  if (isEditMode && !editMode) {
                    setEditMode(EditMode.SelectBorder);
                    setSelectedBorderPair(index);
                    event.stopPropagation(); // To prevent click from unselecting border
                  }
                }}
              >
                {displayLinkedBorder(getMonitorInfo(border, editedAppsMonitors), border, { color: colorToHex(pairedBorder) })}
                {selectedBorderPair === index && displayLinkedBorder(
                  getMonitorInfo(border, editedAppsMonitors),
                  border,
                  { pattern: 'hatch' }
                )}
              </g>
            ))
          ))}

          {selectedApp && ( /* Set of monitors translation */
            <>
              {(() => {
                let appMonitors = editedAppsMonitors.find((app) => app.id === selectedApp);

                if (!appsMonitors) {
                  return null;
                }
                appMonitors = appMonitors as AppMonitors;

                return (
                  <g
                    transform={`translate(${svgCursorX - getRelativeMiddleX(appMonitors)} ${svgCursorY - getRelativeMiddleY(appMonitors)})`}
                    style={{ pointerEvents: 'all' }}
                    onClick={(event) => {
                      if (isEditMode && selectedApp) {
                        appMonitors.offset_x = svgCursorX - getRelativeMiddleX(appMonitors);
                        appMonitors.offset_y = svgCursorY - getRelativeMiddleY(appMonitors);
                        setEditMode(undefined);
                        setSelectedApp('');
                        event.stopPropagation(); /* To prevent border selection from being unselected */
                      }
                    }}
                  >
                    <g>
                      {appMonitors.monitors.map((monitor) => (
                        <g>
                          <rect width={`${monitor.width}`} height={`${monitor.height}`} x={`${monitor.x}`} y={`${monitor.y}`} rx="2" ry="2" opacity="0.8" /* Helps users to distinguish superimposed monitors */ fill={`${colorToHex(monitor)}`} />
                          <text
                            x={`${monitor.x + getSvgFontDX(appMonitors.name, monitor.width, monitor.height)}`}
                            y={`${monitor.y + getSvgFontDY(appMonitors.name, monitor.width, monitor.height)}`}
                            className="monitor-label"
                            fontSize={getSvgFontSize(appMonitors.name, monitor.width, monitor.height)}
                          >
                            {appMonitors.name}
                          </text>
                        </g>
                      ))}
                    </g>
                    <rect width="2000" height="2000" x="-1000" y="-1000" opacity="0" /* Invisible rectangle to ensure group is always clickable */ />
                  </g>
                );
              })()}
            </>
          )}

          {selectedBorderPair !== undefined && ( /* Selected border options, including deletion */
            <>
              {(() => {
                // Place a trash icon at the center between the centers of two monitors
                const app1 = editedAppsMonitors.find((app) => app.id === borders[selectedBorderPair].pair[0].app_id);
                const monitor1 = app1?.monitors?.[borders[selectedBorderPair].pair[0].monitor_index];

                const app2 = editedAppsMonitors.find((app) => app.id === borders[selectedBorderPair].pair[1].app_id);
                const monitor2 = app2?.monitors?.[borders[selectedBorderPair].pair[1].monitor_index];

                let width1 = 400;
                let height1 = 400;
                let width2 = 400;
                let height2 = 400;

                let x = 0;
                let y = 0;
                if (app1 && monitor1) {
                  x += app1.offset_x + monitor1.x + monitor1.width / 2;
                  y += app1.offset_y + monitor1.y + monitor1.height / 2;
                  width1 = monitor1.width;
                  height1 = monitor1.height;
                }
                if (app2 && monitor2) {
                  x += app2.offset_x + monitor2.x + monitor2.width / 2;
                  y += app2.offset_y + monitor2.y + monitor2.height / 2;
                  width2 = monitor2.width;
                  height2 = monitor2.height;
                }

                if (monitor1 && monitor2) {
                  x /= 2;
                  y /= 2;
                }
                const buttonDiameter = 0.75 * Math.min(width1, height1, width2, height2);
                x -= 0.5 * buttonDiameter;
                y -= 0.5 * buttonDiameter;

                return (
                  <foreignObject
                    x={`${x}`}
                    y={`${y}`}
                    width={`${buttonDiameter}`}
                    height={`${buttonDiameter}`}>
                    <button
                      onClick={() => {
                        borders.splice(selectedBorderPair, 1);
                        setEditMode(undefined);
                      }}
                      style={{ border: `${0.06 * buttonDiameter}px solid`, borderRadius: '50%', width: '100%', height: '100%'}}
                      className="svg-delete-border-pair-button"
                    >
                      <TrashIcon style={{ width: '100%', height: '100%'}} />
                    </button>
                  </foreignObject>
                )
              })()}
            </>
          )}

          {!isEditMode && hoveredOnMonitor && (
            /* Marks where the cursor lands on the machine you are about to click. */
            <circle
              cx={`${svgCursorX}`}
              cy={`${svgCursorY}`}
              r={`${30}`}
              className="monitor-cursor-target"
              style={{ pointerEvents: 'none' }}
            >
            </circle>
          )}
        </svg>
      </div>
    </>
  );
};

export default MonitorsViewer;