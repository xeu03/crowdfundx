import { Link, NavLink } from 'react-router-dom';
import { WalletButton } from './WalletButton';
import type { WalletState } from '../hooks/useWallet';

export function Header({ wallet }: { wallet: WalletState }) {
  return (
    <header className="header">
      <div className="container header__inner">
        <Link to="/" className="header__brand">
          <span className="header__logo" aria-hidden="true">
            ◈
          </span>
          CrowdfundX
        </Link>
        <nav className="header__nav" aria-label="Main">
          <NavLink
            to="/"
            end
            className={({ isActive }) => `header__link ${isActive ? 'header__link--active' : ''}`}
          >
            Explore
          </NavLink>
          <NavLink
            to="/create"
            className={({ isActive }) => `header__link ${isActive ? 'header__link--active' : ''}`}
          >
            Start a campaign
          </NavLink>
        </nav>
        <WalletButton wallet={wallet} />
      </div>
    </header>
  );
}
