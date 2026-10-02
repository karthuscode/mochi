import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { Companion } from './Companion';
import './companion.css';

const root = document.getElementById('root');
if (root === null) throw new Error('Companion root is missing');
createRoot(root).render(
  <StrictMode>
    <Companion />
  </StrictMode>,
);
