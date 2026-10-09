import { useState } from 'react';
import { ZoomIn, ZoomOut } from 'lucide-react';
import { Dialog } from './Dialog';

export function ImagePreview({
  name,
  src,
  error,
  close,
}: {
  name: string;
  src?: string;
  error?: string;
  close: () => void;
}) {
  const [actualSize, setActualSize] = useState(false);
  const [failed, setFailed] = useState(false);
  return (
    <Dialog title={name} close={close} wide>
      {error || failed ? (
        <p className="notice error" role="alert">
          {error || 'Не удалось отобразить изображение'}
        </p>
      ) : !src ? (
        <p role="status" className="muted">
          Загружаю изображение…
        </p>
      ) : (
        <>
          <div className="image-preview-toolbar">
            <span className="muted small">
              {actualSize ? 'Исходный размер' : 'По размеру окна'}
            </span>
            <button
              className="secondary-button"
              aria-pressed={actualSize}
              onClick={() => setActualSize(!actualSize)}
            >
              {actualSize ? <ZoomOut size={16} /> : <ZoomIn size={16} />}
              {actualSize ? 'Вписать в окно' : 'Исходный размер'}
            </button>
          </div>
          <div className={`image-preview ${actualSize ? 'actual-size' : ''}`}>
            <img src={src} alt={name} onError={() => setFailed(true)} />
          </div>
        </>
      )}
    </Dialog>
  );
}
