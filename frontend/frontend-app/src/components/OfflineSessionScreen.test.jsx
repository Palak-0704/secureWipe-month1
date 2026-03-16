import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import OfflineSessionScreen from './OfflineSessionScreen';

function jsonResponse(body) {
  return {
    ok: true,
    json: async () => body,
  };
}

describe('OfflineSessionScreen certificate evidence', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders verification evidence details for a reviewed session', async () => {
    const session = {
      session_id: 'sess-1',
      created_at: '2026-01-01T00:00:00Z',
      mode: 'offline',
      target_device_id: 'disk1',
      target_device_model: 'Model X',
      target_device_size_gb: 512,
      method: 'overwrite',
      phase: 'completed',
      progress_percent: 100,
    };

    const certReview = {
      status: 'certificate_review_ready',
      wipe_id: 'sess-1',
      manifest_phase: 'completed',
      completion_status: 'verified',
      verification_passed: true,
      certificate_eligible: true,
      signature_verified: true,
      recommended_action: 'Certificate may be distributed to users or downstream systems.',
      issues: [],
      verification_evidence: {
        sample_blocks_checked: 8,
        sample_blocks_anomalies: 0,
        checksum_algorithm: 'sha256',
        verification_tool: 'integration-test',
        operator_id: 'tester',
      },
    };

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
      if (String(url).includes('/api/wipe/sessions')) {
        return jsonResponse([session]);
      }
      if (String(url).includes('/api/usb/devices')) {
        return jsonResponse([]);
      }
      if (String(url).includes('/api/certificate/sess-1/review')) {
        return jsonResponse(certReview);
      }
      return jsonResponse([]);
    });

    render(
      <OfflineSessionScreen
        devices={[
          {
            id: 'disk1',
            model: 'Model X',
            size_gb: 512,
          },
        ]}
      />,
    );

    await screen.findByText('sess-1');
    fireEvent.click(screen.getByText('sess-1'));

    await waitFor(() => {
      expect(screen.getByText('Verification Evidence')).toBeInTheDocument();
    });

    expect(screen.getByText('Blocks Checked')).toBeInTheDocument();
    expect(screen.getByText('Anomalies')).toBeInTheDocument();
    expect(screen.getByText('Algorithm')).toBeInTheDocument();
    expect(screen.getByText('Tool')).toBeInTheDocument();
    expect(screen.getByText('Operator')).toBeInTheDocument();

    expect(screen.getByText('8')).toBeInTheDocument();
    expect(screen.getByText('0')).toBeInTheDocument();
    expect(screen.getByText('sha256')).toBeInTheDocument();
    expect(screen.getByText('integration-test')).toBeInTheDocument();
    expect(screen.getByText('tester')).toBeInTheDocument();
  });

  it('surfaces anomaly detector issues in certificate review', async () => {
    const session = {
      session_id: 'sess-2',
      created_at: '2026-01-01T00:00:00Z',
      mode: 'offline',
      target_device_id: 'disk1',
      target_device_model: 'Model X',
      target_device_size_gb: 512,
      method: 'overwrite',
      phase: 'failed',
      progress_percent: 90,
    };

    const certReview = {
      status: 'certificate_review_attention_required',
      wipe_id: 'sess-2',
      manifest_phase: 'failed',
      completion_status: 'verified',
      verification_passed: true,
      certificate_eligible: false,
      signature_verified: true,
      recommended_action: 'Review required.',
      issues: ['Anomaly detector: verification notes contain anomaly/error keywords for a verified result.'],
      verification_evidence: {
        sample_blocks_checked: 8,
        sample_blocks_anomalies: 0,
        checksum_algorithm: 'sha256',
        verification_tool: 'integration-test',
        operator_id: 'tester',
      },
    };

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (url) => {
      if (String(url).includes('/api/wipe/sessions')) {
        return jsonResponse([session]);
      }
      if (String(url).includes('/api/usb/devices')) {
        return jsonResponse([]);
      }
      if (String(url).includes('/api/certificate/sess-2/review')) {
        return jsonResponse(certReview);
      }
      return jsonResponse([]);
    });

    render(
      <OfflineSessionScreen
        devices={[
          {
            id: 'disk1',
            model: 'Model X',
            size_gb: 512,
          },
        ]}
      />,
    );

    await screen.findByText('sess-2');
    fireEvent.click(screen.getByText('sess-2'));

    await waitFor(() => {
      expect(screen.getByText(/Anomaly detector:/)).toBeInTheDocument();
    });
  });
});
