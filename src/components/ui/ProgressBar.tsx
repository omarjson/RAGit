export function ProgressBar({ percent }: { percent: number }) {
  return (
    <div className="h-1.5 w-full overflow-hidden rounded-sm bg-[#1C1E32]">
      <div
        className="h-full bg-brand-fg transition-all duration-300 ease-out"
        style={{ width: `${Math.min(percent, 100)}%` }}
      />
    </div>
  );
}

export function ProgressBarShimmer({ percent }: { percent: number }) {
  return (
    <div className="h-1.5 w-full overflow-hidden rounded-sm bg-[#1C1E32]">
      <div
        className="h-full animate-shimmer transition-all duration-300 ease-out"
        style={{
          width: `${Math.min(percent, 100)}%`,
          background: 'linear-gradient(90deg, #F0B429 0%, #F5C842 50%, #F0B429 100%)',
          backgroundSize: '200% 100%',
        }}
      />
    </div>
  );
}
