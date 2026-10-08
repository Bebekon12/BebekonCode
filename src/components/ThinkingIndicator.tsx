export function ThinkingIndicator({ label = 'Обдумывает задачу…' }: { label?: string }) {
  return (
    <span className="thinking-indicator" role="status">
      <span className="thinking-orbit" aria-hidden="true">
        <i />
        <i />
        <i />
      </span>
      <span className="thinking-label">{label}</span>
    </span>
  );
}
