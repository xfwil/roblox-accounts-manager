import { useEffect, useId, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { Icon } from './Icon';

export function Modal({ title, description, onClose, children, className = '' }: {
  title: string; description?: string; onClose: () => void; children: ReactNode; className?: string;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const pressedBackdrop = useRef(false);
  useEffect(() => {
    const dialog = ref.current;
    const previous = document.activeElement;
    dialog?.showModal();
    return () => {
      dialog?.close();
      if (previous instanceof HTMLElement) previous.focus();
    };
  }, []);
  return createPortal(<dialog ref={ref} className={`modal ${className}`} aria-labelledby={titleId} onCancel={event => { event.preventDefault(); onClose(); }} onMouseDown={event => { pressedBackdrop.current = event.target === event.currentTarget; }} onClick={event => {
    if (event.target !== event.currentTarget || !pressedBackdrop.current) return;
    const bounds = event.currentTarget.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom) onClose();
  }}>
    <header className="modal-heading"><div><h2 id={titleId}>{title}</h2>{description && <p>{description}</p>}</div><button type="button" className="btn-icon" onClick={onClose} aria-label="Close dialog"><Icon name="close" /></button></header>
    {children}
  </dialog>, document.body);
}
