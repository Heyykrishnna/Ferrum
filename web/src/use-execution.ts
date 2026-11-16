import { useEffect, useRef, useState } from 'react';

import type { CreateSubmission, Submission } from '../shared/contracts';
import { isPending } from '../shared/contracts';
import {
  cancelSubmission,
  createSubmission,
  errorMessage,
  fetchSubmission,
} from './api';

type Mode = 'run' | 'submit';

interface Attempt {
  mode: Mode;
  key: string;
  payload: CreateSubmission;
}

const pollInterval = 1000;
const pollFor = 90_000;

export function useExecution() {
  const [attempt, setAttempt] = useState<Attempt>();
  const [submission, setSubmission] = useState<Submission>();
  const [error, setError] = useState<string>();
  const [status, setStatus] = useState<
    'idle' | 'sending' | 'polling' | 'paused'
  >('idle');
  const [cancelling, setCancelling] = useState(false);
  const requestInFlight = useRef(false);
  const pending = submission !== undefined && isPending(submission);
  const pollingId = status === 'polling' && pending ? submission.id : undefined;

  useEffect(() => {
    if (!pollingId) return;
    const controller = new AbortController();
    const deadline = Date.now() + pollFor;
    let timer: ReturnType<typeof setTimeout>;

    const poll = async () => {
      try {
        const result = await fetchSubmission(pollingId, controller.signal);
        if (controller.signal.aborted) return;
        setSubmission(result);
        if (!isPending(result)) {
          setStatus('idle');
          return;
        }
        if (Date.now() >= deadline) {
          setError(
            'Still waiting for a worker. Refresh the result to keep checking.',
          );
          setStatus('paused');
          return;
        }
        timer = setTimeout(() => void poll(), pollInterval);
      } catch (cause) {
        if (controller.signal.aborted) return;
        setError(errorMessage(cause));
        setStatus('paused');
      }
    };

    timer = setTimeout(() => void poll(), pollInterval);
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [pollingId]);

  async function send(nextAttempt: Attempt) {
    if (requestInFlight.current || pending) return;
    requestInFlight.current = true;
    setAttempt(nextAttempt);
    setSubmission(undefined);
    setError(undefined);
    setStatus('sending');
    try {
      const result = await createSubmission(
        nextAttempt.payload,
        nextAttempt.key,
      );
      setSubmission(result);
      setStatus(isPending(result) ? 'polling' : 'idle');
    } catch (cause) {
      setError(errorMessage(cause));
      setStatus('idle');
    } finally {
      requestInFlight.current = false;
    }
  }

  function refresh() {
    setError(undefined);
    setStatus('polling');
  }

  async function cancel() {
    if (!submission || !pending || cancelling) return;
    setCancelling(true);
    setError(undefined);
    try {
      const result = await cancelSubmission(submission.id);
      if (result.outcome !== 'cancellation_requested') {
        const updated = await fetchSubmission(
          submission.id,
          AbortSignal.timeout(15_000),
        );
        setSubmission(updated);
        setStatus(isPending(updated) ? 'polling' : 'idle');
      } else {
        setStatus('polling');
        return;
      }
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setCancelling(false);
    }
  }

  return {
    mode: attempt?.mode,
    submission,
    status,
    error,
    pending,
    cancelling,
    send,
    cancel,
    refresh,
    retry:
      error && !submission && attempt ? () => void send(attempt) : undefined,
  };
}
