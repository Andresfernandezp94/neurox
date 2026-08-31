// Tests del ConnectionIndicator. EP-0003-03.

import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';
import { StoreProvider } from '../store/StoreContext';
import { ConnectionIndicator } from './ConnectionIndicator';

function withState(node: React.ReactNode) {
  return render(
    <StoreProvider eventsPath="/__test_no_ws__">{node}</StoreProvider>,
  );
}

describe('ConnectionIndicator', () => {
  it('renders without crashing inside StoreProvider', () => {
    const { getByTestId } = withState(<ConnectionIndicator />);
    const el = getByTestId('connection-indicator');
    // El WS está en connecting al inicio, health null → 'degraded' o
    // el estado que computeCombined determine con esos inputs.
    expect(['ok', 'degraded', 'offline']).toContain(el.getAttribute('data-combined'));
  });
});
