import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Companion } from './Companion';

const native = vi.hoisted(() => ({
  startCompanionDrag: vi.fn(),
  openMochi: vi.fn(),
  hideCompanion: vi.fn(),
}));
vi.mock('./native/companion', () => native);

beforeEach(() => {
  vi.stubGlobal('PointerEvent', MouseEvent);
  native.startCompanionDrag.mockReset().mockResolvedValue(undefined);
  native.openMochi.mockReset().mockResolvedValue(undefined);
  native.hideCompanion.mockReset().mockResolvedValue(undefined);
});

function drag(button: HTMLElement) {
  fireEvent.pointerDown(button, { button: 0, clientX: 40, clientY: 40 });
  fireEvent.pointerMove(button, { buttons: 1, clientX: 60, clientY: 40 });
  fireEvent.pointerUp(button);
}

describe('desktop companion interaction', () => {
  it('opens the app on a click with small pointer jitter', async () => {
    render(<Companion />);
    const button = screen.getByRole('button', { name: 'Open mochi' });
    fireEvent.pointerDown(button, { button: 0, clientX: 40, clientY: 40 });
    fireEvent.pointerMove(button, { buttons: 1, clientX: 42, clientY: 40 });
    fireEvent.pointerUp(button);
    fireEvent.click(button, { detail: 1 });
    await waitFor(() => expect(native.openMochi).toHaveBeenCalledOnce());
    expect(native.startCompanionDrag).not.toHaveBeenCalled();
  });

  it('moves the window once and suppresses the click produced by a drag', () => {
    render(<Companion />);
    const button = screen.getByRole('button', { name: 'Open mochi' });
    drag(button);
    fireEvent.pointerMove(button, { buttons: 1, clientX: 90, clientY: 40 });
    fireEvent.click(button, { detail: 1 });
    expect(native.startCompanionDrag).toHaveBeenCalledOnce();
    expect(native.openMochi).not.toHaveBeenCalled();
    fireEvent.click(button, { detail: 1 });
    expect(native.openMochi).toHaveBeenCalledOnce();
  });

  it('keeps keyboard activation available after a drag without a click', () => {
    render(<Companion />);
    const button = screen.getByRole('button', { name: 'Open mochi' });
    drag(button);
    fireEvent.click(button, { detail: 0 });
    expect(native.openMochi).toHaveBeenCalledOnce();
  });

  it('reports a failed native drag without opening the app', async () => {
    native.startCompanionDrag.mockRejectedValue(
      new Error('private native detail'),
    );
    render(<Companion />);
    const button = screen.getByRole('button', { name: 'Open mochi' });
    drag(button);
    fireEvent.click(button, { detail: 1 });
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Use mochi’s Dock icon.',
    );
    expect(native.openMochi).not.toHaveBeenCalled();
    expect(screen.queryByText('private native detail')).not.toBeInTheDocument();
  });

  it('retains an accessible opener if the character image fails', () => {
    render(<Companion />);
    const image = document.querySelector('img');
    if (!image) throw new Error('Expected character image');
    fireEvent.error(image);
    fireEvent.click(screen.getByRole('button', { name: 'Open mochi' }));
    expect(native.openMochi).toHaveBeenCalledOnce();
  });
});
