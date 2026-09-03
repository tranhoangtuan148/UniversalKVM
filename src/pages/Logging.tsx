import { useContext, useEffect, useRef, useState } from "react";
import * as ScrollArea from "@radix-ui/react-scroll-area";
import { invoke } from "@tauri-apps/api/core";
import { debug } from '@tauri-apps/plugin-log';
import { GlobalContext } from "../App";
import WarnText from "../components/Warn";
import "./Logging.css";


function Logging() {
  const global = useContext(GlobalContext);

  const [maxLogs, setMaxLogs] = useState<number>(global.maximum_logs);

  useEffect(() => {
    setMaxLogs(global.maximum_logs);
  }, [global.maximum_logs]);

  /* New lines arrive at the bottom, so the view follows them. */
  const viewportRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const viewport = viewportRef.current;
    if (viewport) {
      viewport.scrollTop = viewport.scrollHeight;
    }
  }, [global.logs.length]);

  const clearLogs = (): void => {
    global.setGlobal({
      logs: [],
    });
  }

  const onChangeMaxLogs = (event: React.ChangeEvent<HTMLInputElement>) => {
    event.preventDefault();
    let { value, min, max } = event.target;
    const maxLogs = Math.max(Number(min), Math.min(Number(max), Number(value)));
    setMaxLogs(maxLogs);
  };

  const onSubmitMaxLogs = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    debug(`invoke submit_config (maximum_logs)`);
    invoke("submit_config", { partial_config: JSON.stringify({ maximum_logs: maxLogs }) });
  };

  /* 1=error, 8=info, 16=debug. Only errors get colour, so they stay findable. */
  const logLevelClass = (level: number): string => {
    if (level <= 1) {
      return 'log-line log-line-error';
    }
    if (level <= 8) {
      return 'log-line';
    }
    return 'log-line log-line-debug';
  };

  const errorCount = global.logs.reduce((count, log) => log.level <= 1 ? count + 1 : count, 0);

  return (
    <div className="logging">
      <div className="eyebrow">
        Session log
        <span className="eyebrow-count">
          {global.logs.length} {global.logs.length === 1 ? 'line' : 'lines'}
          {errorCount > 0 && <span className="error-red">{` · ${errorCount} ${errorCount === 1 ? 'error' : 'errors'}`}</span>}
        </span>
      </div>

      <ScrollArea.Root type="always" className="scroll-area-root">
        <ScrollArea.Viewport className="scroll-area-viewport" ref={viewportRef}>
          {global.logs.length === 0 ? (
            <div className="log-empty">
              Nothing logged yet. Connecting to a machine or capturing a device writes here.
            </div>
          ) : global.logs.map((log, index) => (
            <div className={logLevelClass(log.level)} key={index}>
              <span className="log-time">{log.tag}</span>
              <span className="log-message">{log.message}</span>
            </div>
          ))}
        </ScrollArea.Viewport>

        <ScrollArea.Scrollbar
          className="scroll-area-scrollbar"
          orientation="vertical"
        >
          <ScrollArea.Thumb className="scroll-area-thumb" />
        </ScrollArea.Scrollbar>
        <ScrollArea.Scrollbar
          className="scroll-area-scrollbar"
          orientation="horizontal"
        >
          <ScrollArea.Thumb className="scroll-area-thumb" />
        </ScrollArea.Scrollbar>
        <ScrollArea.Corner className="scroll-area-corner" />
      </ScrollArea.Root>

      <div className="logging-footer">
        <form onSubmit={onSubmitMaxLogs} className="max-logs">
          <label className="setting-label" htmlFor="max-logs">Keep at most</label>
          <input
            id="max-logs"
            value={maxLogs}
            onChange={onChangeMaxLogs}
            type="number"
            min={1}
            max={100000}
          />
          <span className="mono">lines</span>
          <WarnText show={maxLogs !== global.maximum_logs} text="Press Enter to apply" />
        </form>

        <button onClick={clearLogs}>Clear log</button>
      </div>

      <p className="logging-note">
        Logs are kept in memory for this session only, and never record keystrokes or
        mouse movement.
      </p>
    </div>
  );
}

export default Logging;
