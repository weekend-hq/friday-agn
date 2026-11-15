import { createSignal, For, onMount, type Component } from 'solid-js';

type ChatMessage = {
  id: string;
  role: 'user' | 'assistant';
  content: string;
};

type MessageResponse = {
  success: boolean;
  data: {
    task_id: string;
    message: string;
  } | null;
  error: {
    message: string;
  } | null;
};

const App: Component = () => {
  const [sessionId] = createSignal(crypto.randomUUID());
  const [messages, setMessages] = createSignal<ChatMessage[]>([
    {
      id: crypto.randomUUID(),
      role: 'assistant',
      content:
        'Hi! Send me a browser task and I’ll pass it to the FRIDAY-AGN backend.',
    },
  ]);
  const [draft, setDraft] = createSignal('');
  const [isSending, setIsSending] = createSignal(false);
  const [backendConnected, setBackendConnected] = createSignal(false);
  const [checkingBackend, setCheckingBackend] = createSignal(true);

  onMount(() => {
    void fetch('/api/v1/system/health')
      .then((response) => setBackendConnected(response.ok))
      .catch(() => setBackendConnected(false))
      .finally(() => setCheckingBackend(false));
  });

  const sendMessage = async (event: SubmitEvent) => {
    event.preventDefault();
    const message = draft().trim();
    if (!message || isSending()) return;

    setMessages((current) => [
      ...current,
      { id: crypto.randomUUID(), role: 'user', content: message },
    ]);
    setDraft('');
    setIsSending(true);

    try {
      const response = await fetch('/api/v1/agent/message', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ session_id: sessionId(), message }),
      });
      const result = (await response.json()) as MessageResponse;

      if (!response.ok || !result.success || !result.data) {
        throw new Error(result.error?.message ?? 'The backend rejected the message.');
      }

      setMessages((current) => [
        ...current,
        {
          id: crypto.randomUUID(),
          role: 'assistant',
          content: `${result.data.message}\nTask ID: ${result.data.task_id}`,
        },
      ]);
    } catch (error) {
      const detail =
        error instanceof Error ? error.message : 'An unexpected error occurred.';
      setMessages((current) => [
        ...current,
        {
          id: crypto.randomUUID(),
          role: 'assistant',
          content: `Could not send your message: ${detail}`,
        },
      ]);
    } finally {
      setIsSending(false);
    }
  };

  return (
    <main class="flex min-h-screen items-center justify-center bg-slate-950 px-4 py-8 text-slate-100">
      <section class="flex h-[min(760px,calc(100vh-4rem))] w-full max-w-3xl flex-col overflow-hidden rounded-2xl border border-slate-800 bg-slate-900 shadow-2xl">
        <header class="border-b border-slate-800 px-6 py-5">
          <div class="flex items-center gap-3">
            <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-400/10 text-lg font-bold text-emerald-300">
              F
            </div>
            <div>
              <h1 class="text-lg font-semibold">FRIDAY-AGN</h1>
              <p class="text-sm text-slate-400">Browser agent chat</p>
            </div>
            <span
              class={`ml-auto flex items-center gap-2 text-xs ${
                checkingBackend()
                  ? 'text-amber-300'
                  : backendConnected()
                    ? 'text-emerald-300'
                    : 'text-rose-300'
              }`}
              role="status"
            >
              <span
                class={`h-2 w-2 rounded-full ${
                  checkingBackend()
                    ? 'bg-amber-400'
                    : backendConnected()
                      ? 'bg-emerald-400'
                      : 'bg-rose-400'
                }`}
              />
              {checkingBackend()
                ? 'Checking backend…'
                : backendConnected()
                  ? 'Backend connected'
                  : 'Backend unavailable'}
            </span>
          </div>
        </header>

        <div
          class="flex-1 space-y-5 overflow-y-auto px-5 py-6 sm:px-7"
          aria-live="polite"
          aria-label="Chat messages"
        >
          <For each={messages()}>
            {(message) => (
              <div
                class={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
              >
                <div
                  class={`max-w-[85%] whitespace-pre-wrap rounded-2xl px-4 py-3 text-sm leading-6 ${
                    message.role === 'user'
                      ? 'rounded-br-sm bg-emerald-400 text-slate-950'
                      : 'rounded-bl-sm bg-slate-800 text-slate-100'
                  }`}
                >
                  {message.content}
                </div>
              </div>
            )}
          </For>
          {isSending() && (
            <div class="text-sm text-slate-400" role="status">
              Sending to backend…
            </div>
          )}
        </div>

        <form
          class="border-t border-slate-800 bg-slate-900 p-4 sm:p-5"
          onSubmit={(event) => void sendMessage(event)}
        >
          <label class="sr-only" for="chat-message">
            Message
          </label>
          <div class="flex items-end gap-3">
            <textarea
              id="chat-message"
              class="max-h-36 min-h-12 flex-1 resize-y rounded-xl border border-slate-700 bg-slate-950 px-4 py-3 text-sm text-slate-100 outline-none placeholder:text-slate-500 focus:border-emerald-400"
              placeholder="Describe a browser task…"
              rows="1"
              value={draft()}
              onInput={(event) => setDraft(event.currentTarget.value)}
              disabled={isSending()}
            />
            <button
              class="h-12 rounded-xl bg-emerald-400 px-5 text-sm font-semibold text-slate-950 transition hover:bg-emerald-300 disabled:cursor-not-allowed disabled:opacity-50"
              type="submit"
              disabled={isSending() || !draft().trim()}
            >
              {isSending() ? 'Sending…' : 'Send'}
            </button>
          </div>
          <p class="mt-3 text-xs text-slate-500">
            Messages are sent to the backend and printed in its terminal. The
            agent does not execute browser tasks yet.
          </p>
        </form>
      </section>
    </main>
  );
};

export default App;
