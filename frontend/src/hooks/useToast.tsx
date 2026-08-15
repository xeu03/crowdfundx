import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';

export type ToastKind = 'success' | 'error' | 'info';

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
}

interface ToastApi {
  push: (kind: ToastKind, message: string) => void;
}

const ToastContext = createContext<ToastApi>({ push: () => {} });

export function useToast(): ToastApi {
  return useContext(ToastContext);
}

const TOAST_TTL_MS = 5_000;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(0);

  const push = useCallback((kind: ToastKind, message: string) => {
    const id = nextId.current++;
    setToasts((current) => [...current, { id, kind, message }]);
    setTimeout(() => {
      setToasts((current) => current.filter((t) => t.id !== id));
    }, TOAST_TTL_MS);
  }, []);

  const api = useMemo(() => ({ push }), [push]);

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div className="toast-host" role="status" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={`toast toast--${t.kind}`} data-testid="toast">
            {t.message}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}
