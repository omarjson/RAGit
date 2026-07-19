import { Component, ReactNode } from "react";

interface State {
  error: Error | null;
}

export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error) {
    // eslint-disable-next-line no-console
    console.error("RAGit crash:", error);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-screen w-screen flex-col gap-3 overflow-auto bg-deep p-8 text-primary">
          <h1 className="text-xl font-semibold text-red-400">RAGit crashed</h1>
          <pre className="whitespace-pre-wrap rounded-sm border border-[#2A2D45] bg-[#141626] p-4 font-mono text-xs text-red-300">
            {String(this.state.error?.stack || this.state.error)}
          </pre>
        </div>
      );
    }
    return this.props.children;
  }
}
