import { useEffect, useRef } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import '@xterm/xterm/css/xterm.css';

interface TerminalProps {
  lines: { type: 'stdout' | 'stderr'; content: string }[];
  exitCode: number | null;
}

export function Terminal({ lines, exitCode }: TerminalProps) {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);

  useEffect(() => {
    if (!terminalRef.current) return;

    const xterm = new XTerm({
      theme: {
        background: '#1a1a2e',
        foreground: '#e0e0e0',
        cursor: '#a0a0a0',
        black: '#000000',
        red: '#ff5555',
        green: '#50fa7b',
        yellow: '#f1fa8c',
        blue: '#bd93f9',
        magenta: '#ff79c6',
        cyan: '#8be9fd',
        white: '#e0e0e0',
        brightBlack: '#555555',
        brightRed: '#ff6e6e',
        brightGreen: '#69ff94',
        brightYellow: '#ffffa5',
        brightBlue: '#d6acff',
        brightMagenta: '#ff92df',
        brightCyan: '#a4ffff',
        brightWhite: '#ffffff',
      },
      fontFamily: '"Cascadia Code", "Fira Code", "JetBrains Mono", monospace',
      fontSize: 13,
      lineHeight: 1.4,
      cursorBlink: true,
      convertEol: true,
    });

    xterm.open(terminalRef.current);
    xtermRef.current = xterm;

    return () => {
      xterm.dispose();
    };
  }, []);

  useEffect(() => {
    const xterm = xtermRef.current;
    if (!xterm) return;

    xterm.clear();

    for (const line of lines) {
      const color = line.type === 'stderr' ? 'red' : 'green';
      xterm.write(`\x1b[${color === 'red' ? '31' : '32'}m${line.content}\x1b[0m\r\n`);
    }

    if (exitCode !== null) {
      const color = exitCode === 0 ? 'green' : 'red';
      const prefix = exitCode === 0 ? '✓' : '✗';
      xterm.write(`\r\n\x1b[${color === 'green' ? '32' : '31'}m${prefix} Process exited with code ${exitCode}\x1b[0m`);
    }
  }, [lines, exitCode]);

  return <div ref={terminalRef} className="terminal-container" />;
}