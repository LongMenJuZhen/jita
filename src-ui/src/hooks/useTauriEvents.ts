import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useAppStore } from '../store/appStore';
import type { OutputLine, TaskComplete } from '../types';

export function useTauriEvents() {
  const {
    appendOutputLine,
    setOutputExitCode,
    setExecuting,
    setStatus,
    setState,
    setTaskId,
    runningTaskId,
  } = useAppStore();

  useEffect(() => {
    const unlistenOutput = listen<OutputLine>('script_output', (event) => {
      if (runningTaskId) {
        appendOutputLine(runningTaskId, {
          type: event.payload.line_type as 'stdout' | 'stderr',
          content: event.payload.content,
        });
      }
    });

    const unlistenComplete = listen<TaskComplete>('task_complete', (event) => {
      const { exit_code, error, task_id } = event.payload;

      if (runningTaskId === task_id) {
        if (task_id) {
          setOutputExitCode(task_id, exit_code);
        }
        setExecuting(false);
        setTaskId('');

        if (exit_code === 0) {
          setStatus('执行成功');
        } else if (exit_code !== null) {
          setStatus(`执行失败，退出码: ${exit_code}`);
        } else {
          setStatus(error || '执行错误');
        }
      }
    });

    const unlistenAsrStatus = listen<string>('asr_status', (event) => {
      console.log('ASR status:', event.payload);
      setStatus(event.payload);
    });

    const unlistenAsr = listen<string>('asr_text', (event) => {
      console.log('ASR text:', event.payload);
      const { inputText, setInputText } = useAppStore.getState();
      if (inputText) {
        setInputText(inputText + ' ' + event.payload);
      } else {
        setInputText(event.payload);
      }
    });

    return () => {
      unlistenOutput.then((fn) => fn());
      unlistenComplete.then((fn) => fn());
      unlistenAsrStatus.then((fn) => fn());
      unlistenAsr.then((fn) => fn());
    };
  }, [runningTaskId, appendOutputLine, setOutputExitCode, setExecuting, setStatus, setState, setTaskId]);
}