import { Component, type ReactNode } from 'react';
import { HashRouter, Route, Routes } from 'react-router-dom';
import { Header } from './components/Header';
import { ErrorState } from './components/ErrorState';
import { useWallet } from './hooks/useWallet';
import { Explore } from './pages/Explore';
import { CampaignDetail } from './pages/CampaignDetail';
import { CreateCampaign } from './pages/CreateCampaign';

/** Catches render errors anywhere below and shows a recoverable state. */
class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <div className="container">
          <ErrorState
            title="The app crashed"
            message={this.state.error.message}
            onRetry={() => this.setState({ error: null })}
          />
        </div>
      );
    }
    return this.props.children;
  }
}

function NotFound() {
  return (
    <div className="container">
      <ErrorState title="404" message="This page doesn't exist." />
    </div>
  );
}

export default function App() {
  const wallet = useWallet();

  return (
    <ErrorBoundary>
      <HashRouter>
        <Header wallet={wallet} />
        <main>
          <Routes>
            <Route path="/" element={<Explore />} />
            <Route path="/campaign/:id" element={<CampaignDetail walletAddress={wallet.address} />} />
            <Route path="/create" element={<CreateCampaign walletAddress={wallet.address} />} />
            <Route path="*" element={<NotFound />} />
          </Routes>
        </main>
        <footer className="footer">
          <div className="container">
            Built on <a href="https://stellar.org/soroban">Stellar Soroban</a> ·{' '}
            <a href="https://github.com/xeu03/crowdfundx">GitHub</a>
          </div>
        </footer>
      </HashRouter>
    </ErrorBoundary>
  );
}
