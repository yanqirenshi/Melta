import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import Sample from './documents/Sample';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Sample />
  </StrictMode>,
);
