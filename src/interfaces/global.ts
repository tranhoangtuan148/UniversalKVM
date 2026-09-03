export interface Devices {
    keyboards: {
        name: string,
        id: string,
        active: boolean,
    }[],
    mouses: {
        name: string,
        id: string,
        active: boolean,
    }[],
}

export interface Log {
  tag: string,
  message: string,
  level: number, // 1=error, 8=info, 16=debug
}

export interface Logging {
    maximum_logs: number, // is configurable
    logs: Log[],
}

export interface MonitorBorder {
    start: number,
    end: number,
    border: number, // 0=left, 1=right, 2=top, 3=bottom
    monitor_index: number,
    monitors_id: string, // Unused on the frontend
    app_id: string,
}

export interface BorderPair {
    pair: MonitorBorder[],
    color_r: number,
    color_g: number,
    color_b: number,
}

export interface Monitor {
    x: number,
    y: number,
    width: number,
    height: number,
    color_r: number,
    color_g: number,
    color_b: number,
}

export interface SetOfMonitors {
    offset_x: number,
    offset_y: number,
    monitors: Monitor[],
}

export interface AppMonitors {
    name: string,
    id: string,
    offset_x: number,
    offset_y: number,
    monitors: Monitor[],
}

export interface FileTransfersProgress {
    timestamp: number,
    finished_bytes: number,
    total_bytes: number,
}

export interface App {
  name: string,
  password: string,
  id: string,
  address_infos: string[],
  set_of_monitors: SetOfMonitors,
  online: boolean,
  focused_id: string,
  authorized_by_self: boolean,
  authorized_by_peer: boolean,
  file_transfers: FileTransfersProgress[],
}

export interface Settings {
    theme: string, // is configurable
    default_width: number, // is configurable
    default_height: number, // is configurable
    zoom: number, // is configurable
    enable_clipboard: boolean, // is configurable
    auto_connect: boolean, // is configurable
    download_path: string, // is configurable
    remembered_keyboards: { id: string, name: string }[], // is configurable
    remembered_mouses: { id: string, name: string }[], // is configurable
}

export interface AppRectangle {
    id: string,
    x1: number,
    y1: number,
    x2: number,
    y2: number
}

export interface Global extends Devices, Logging, Settings {
    isDragActive: boolean,
    dragAppId: string,
    dragPaths: string[],
    dragAppRectangles: AppRectangle[],
}

export interface GlobalContextType extends Global {
    setGlobal: (newGlobal: Partial<Global>) => void;
}