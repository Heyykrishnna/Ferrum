import * as monaco from 'monaco-editor/editor/editor.api';
import EditorWorker from 'monaco-editor/editor/editor.worker.js?worker';
import 'monaco-editor/features/bracketMatching/register.js';
import 'monaco-editor/features/comment/register.js';
import 'monaco-editor/features/contextmenu/register.js';
import 'monaco-editor/features/find/register.js';
import 'monaco-editor/features/folding/register.js';
import 'monaco-editor/languages/definitions/cpp/register.js';
import 'monaco-editor/languages/definitions/java/register.js';
import 'monaco-editor/languages/definitions/javascript/register.js';
import 'monaco-editor/languages/definitions/python/register.js';
import 'monaco-editor/languages/definitions/rust/register.js';

import { useEffect, useEffectEvent, useRef, useState } from 'react';

import type { Language } from '../shared/contracts';

self.MonacoEnvironment = { getWorker: () => new EditorWorker() };

monaco.editor.defineTheme('ferrum-light', {
  base: 'vs',
  inherit: true,
  rules: [
    { token: 'keyword', foreground: '573DA1' },
    { token: 'comment', foreground: '657368', fontStyle: 'italic' },
    { token: 'string', foreground: '1B6649' },
    { token: 'number', foreground: '9A4926' },
  ],
  colors: {
    'editor.background': '#FCFBF7',
    'editor.foreground': '#24332E',
    'editorLineNumber.foreground': '#6F7772',
    'editorLineNumber.activeForeground': '#285142',
    'editor.lineHighlightBackground': '#F0EFE7',
    'editor.selectionBackground': '#D7E8DA',
    'editorCursor.foreground': '#285142',
    'editorIndentGuide.background1': '#E7E5DC',
  },
});
monaco.editor.defineTheme('ferrum-dark', {
  base: 'vs-dark',
  inherit: true,
  rules: [
    { token: 'keyword', foreground: 'C4AEFD' },
    { token: 'comment', foreground: '91A398', fontStyle: 'italic' },
    { token: 'string', foreground: 'A6D8B3' },
    { token: 'number', foreground: 'F1B289' },
  ],
  colors: {
    'editor.background': '#19221F',
    'editor.foreground': '#E2E9E2',
    'editorLineNumber.foreground': '#81988C',
    'editorLineNumber.activeForeground': '#ADE5C2',
    'editor.lineHighlightBackground': '#232E29',
    'editor.selectionBackground': '#355143',
    'editorCursor.foreground': '#ADE5C2',
    'editorIndentGuide.background1': '#34413B',
  },
});

interface EditorProps {
  source: string;
  language: Language;
  dark: boolean;
  onSource: (value: string) => void;
  onPosition: (line: number, column: number) => void;
  onRun: () => void;
}

export default function CodeEditor({
  source,
  language,
  dark,
  onSource,
  onPosition,
  onRun,
}: EditorProps) {
  const container = useRef<HTMLDivElement>(null);
  const [initial] = useState(source);
  const initialTheme = useRef(dark ? 'ferrum-dark' : 'ferrum-light');
  const changeSource = useEffectEvent(onSource);
  const changePosition = useEffectEvent(onPosition);
  const run = useEffectEvent(onRun);

  useEffect(() => {
    if (!container.current) return;
    const editor = monaco.editor.create(container.current, {
      value: initial,
      language,
      theme: initialTheme.current,
      ariaLabel: 'Source code',
      accessibilitySupport: 'auto',
      automaticLayout: true,
      minimap: { enabled: false },
      fontFamily: '"IBM Plex Mono", monospace',
      fontSize: 14,
      lineHeight: 25,
      padding: { top: 20, bottom: 24 },
      scrollBeyondLastLine: false,
      tabSize: 4,
      wordWrap: 'on',
      lineNumbersMinChars: 3,
      roundedSelection: false,
      glyphMargin: false,
      lineDecorationsWidth: 18,
      scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
      unicodeHighlight: { ambiguousCharacters: false },
    });
    const model = editor.getModel();
    const content = editor.onDidChangeModelContent(() =>
      changeSource(editor.getValue()),
    );
    const position = editor.onDidChangeCursorPosition(({ position }) =>
      changePosition(position.lineNumber, position.column),
    );
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () =>
      run(),
    );
    return () => {
      content.dispose();
      position.dispose();
      editor.dispose();
      model?.dispose();
    };
  }, [initial, language]);

  useEffect(() => {
    monaco.editor.setTheme(dark ? 'ferrum-dark' : 'ferrum-light');
  }, [dark]);

  return <div ref={container} className="absolute inset-0" />;
}
