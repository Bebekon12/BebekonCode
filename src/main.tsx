import React from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import './styles.css';
import './chat-polish.css';
import './workspace-design.css';
// Apply saved appearance before the first React paint, without storing provider data here.
try {
  const theme = JSON.parse(localStorage.getItem('bebekoncode.ui.theme') ?? '"dark"');
  const size = JSON.parse(localStorage.getItem('bebekoncode.ui.text-size') ?? '"comfortable"');
  document.documentElement.dataset.theme = theme === 'light' ? 'light' : 'dark';
  document.documentElement.dataset.textSize = size === 'large' ? 'large' : 'comfortable';
} catch {
  /* Default appearance stays readable when local storage is unavailable. */
}
createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
