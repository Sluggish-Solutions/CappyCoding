<script lang="ts">
        import { derived, get, writable } from 'svelte/store'
        import LiveKitVoiceConsole from '$lib/components/LiveKitVoiceConsole.svelte'
        import { taurpc } from '$lib/tauri'
        import type { ClaudeMetricsSnapshot, LivekitTokenResponse } from '../types'

        const metrics = writable<ClaudeMetricsSnapshot | null>(null)
        const metricsLoading = writable(false)
        const metricsError = writable('')

        const livekitToken = writable('')
        const livekitError = writable('')
        const livekitExpiresAt = writable('')

        // @ts-expect-error TauRPC namespaces are keyed by an empty string
        const rpc = taurpc['']

        let dataDir = ''
        let hoursBack = 24
        let pythonPath = ''
        let serverUrl = 'http://localhost:8080'
        let authToken = ''

        let livekitServerUrl = ''
        let livekitApiKey = ''
        let livekitApiSecret = ''
        let livekitIdentity = ''
        let livekitRoom = ''
        let livekitName = ''
        let livekitMetadata = ''
        let livekitTtl = 3600
        let canPublish = true
        let canSubscribe = true
        let canPublishData = true

        let voiceAgentApiKey = ''
        let voiceAgentUrl = ''
        let voiceAgentId = ''
        let voiceAgentVoice = ''

        const metricsTotals = derived(metrics, ($metrics) => {
                if (!$metrics) return null
                return [
                        {
                                label: 'Total Cost (USD)',
                                value: formatCurrency($metrics.total_cost_usd),
                        },
                        {
                                label: 'Burn Rate (USD/hour)',
                                value: formatCurrency($metrics.burn_rate_per_hour),
                        },
                        {
                                label: 'Total Tokens',
                                value: formatNumber($metrics.total_tokens),
                        },
                        {
                                label: 'Sessions',
                                value: String($metrics.session_count),
                        },
                ]
        })

        function formatNumber(value: number) {
                if (!Number.isFinite(value)) return '0'
                return new Intl.NumberFormat().format(value)
        }

        function formatCurrency(value: number) {
                if (!Number.isFinite(value)) return '$0.00'
                return new Intl.NumberFormat(undefined, {
                        style: 'currency',
                        currency: 'USD',
                }).format(value)
        }

        async function loadMetrics() {
                metricsLoading.set(true)
                metricsError.set('')
                try {
                        const result = await rpc.collect_claude_metrics({
                                data_dir: dataDir || null,
                                hours_back: Number.isFinite(hoursBack) ? hoursBack : null,
                                python_path: pythonPath || null,
                        })
                        metrics.set(result)
                } catch (error) {
                        metrics.set(null)
                        metricsError.set(error instanceof Error ? error.message : String(error))
                } finally {
                        metricsLoading.set(false)
                }
        }

        async function syncMetrics() {
                const current = get(metrics)
                if (!current) {
                        metricsError.set('Collect metrics before syncing.')
                        return
                }

                metricsLoading.set(true)
                metricsError.set('')
                try {
                        const result = await rpc.push_claude_metrics({
                                metrics: current,
                                server_url: serverUrl,
                                auth_token: authToken || null,
                        })
                        metrics.set(result)
                } catch (error) {
                        metricsError.set(error instanceof Error ? error.message : String(error))
                } finally {
                        metricsLoading.set(false)
                }
        }

        async function generateLivekitToken(): Promise<LivekitTokenResponse | undefined> {
                livekitError.set('')
                livekitToken.set('')
                livekitExpiresAt.set('')
                try {
                        const result = await rpc.generate_livekit_token({
                                api_key: livekitApiKey,
                                api_secret: livekitApiSecret,
                                identity: livekitIdentity,
                                room: livekitRoom,
                                name: livekitName || null,
                                metadata: livekitMetadata || null,
                                ttl_seconds: Number.isFinite(livekitTtl) ? livekitTtl : null,
                                can_publish: canPublish,
                                can_subscribe: canSubscribe,
                                can_publish_data: canPublishData,
                        })
                        livekitToken.set(result.token)
                        livekitExpiresAt.set(new Date(result.expires_at).toLocaleString())
                        return result
                } catch (error) {
                        livekitError.set(error instanceof Error ? error.message : String(error))
                        return undefined
                }
        }
</script>


<svelte:head>
        <title>Claude Code & Live Metrics Control Center</title>
</svelte:head>

<main class="page">
        <section class="panel">
                <header>
                        <h1>Claude Usage Metrics</h1>
                        <p>
                                Gather usage data from the Claude desktop client using the
                                <code>claude-monitor</code> toolkit and push it to the Golang metrics
                                server for the ESP32 dashboard.
                        </p>
                </header>

                <div class="grid">
                        <label>
                                Data directory
                                <input
                                        placeholder="~/.claude/projects"
                                        bind:value={dataDir}
                                />
                        </label>
                        <label>
                                Lookback hours
                                <input
                                        type="number"
                                        min="1"
                                        bind:value={hoursBack}
                                />
                        </label>
                        <label>
                                Python executable
                                <input
                                        placeholder="python3"
                                        bind:value={pythonPath}
                                />
                        </label>
                        <label>
                                Metrics server URL
                                <input
                                        placeholder="http://localhost:8080"
                                        bind:value={serverUrl}
                                />
                        </label>
                        <label>
                                Server auth token (optional)
                                <input
                                        placeholder="Bearer token"
                                        bind:value={authToken}
                                />
                        </label>
                </div>

                <div class="actions">
                        <button on:click={loadMetrics} disabled={$metricsLoading}>
                                {$metricsLoading ? 'Collecting…' : 'Collect metrics'}
                        </button>
                        <button on:click={syncMetrics} disabled={$metricsLoading || !$metrics}>
                                Push to server
                        </button>
                </div>

                {#if $metricsError}
                        <p class="error">{$metricsError}</p>
                {/if}

                {#if $metrics}
                        <div class="metrics">
                                <div class="totals">
                                        {#each $metricsTotals ?? [] as stat}
                                                <article>
                                                        <h3>{stat.label}</h3>
                                                        <p>{stat.value}</p>
                                                </article>
                                        {/each}
                                </div>
                                <div class="details">
                                        <div>
                                                <h4>Token breakdown</h4>
                                                <ul>
                                                        <li>Input tokens: {formatNumber($metrics.input_tokens)}</li>
                                                        <li>Output tokens: {formatNumber($metrics.output_tokens)}</li>
                                                        <li>Cache creation tokens: {formatNumber($metrics.cache_creation_tokens)}</li>
                                                        <li>Cache read tokens: {formatNumber($metrics.cache_read_tokens)}</li>
                                                </ul>
                                        </div>
                                        <div>
                                                <h4>Timeline</h4>
                                                <ul>
                                                        <li>Window: {$metrics.window_hours.toFixed(2)} hours</li>
                                                        <li>Last activity: {new Date($metrics.last_activity).toLocaleString()}</li>
                                                        <li>Snapshot at: {new Date($metrics.timestamp).toLocaleString()}</li>
                                                        <li>Active session: {$metrics.active_session_id ?? 'n/a'}</li>
                                                </ul>
                                        </div>
                                </div>
                        </div>
                {/if}
        </section>

                <section class="panel">
                <header>
                        <h2>LiveKit Voice Rubber Ducking</h2>
                        <p>
                                Join a LiveKit room, dictate your debugging questions, and listen to spoken
                                answers from the Claude Agent SDK without leaving your editor.
                        </p>
                </header>

                <div class="grid">
                        <label>
                                LiveKit server URL
                                <input
                                        placeholder="wss://localhost:7880"
                                        bind:value={livekitServerUrl}
                                />
                        </label>
                        <label>
                                LiveKit API key
                                <input bind:value={livekitApiKey} />
                        </label>
                        <label>
                                LiveKit API secret
                                <input type="password" bind:value={livekitApiSecret} autocomplete="off" />
                        </label>
                        <label>
                                Identity
                                <input bind:value={livekitIdentity} />
                        </label>
                        <label>
                                Room
                                <input bind:value={livekitRoom} />
                        </label>
                        <label>
                                Display name
                                <input bind:value={livekitName} />
                        </label>
                        <label>
                                Metadata
                                <input bind:value={livekitMetadata} />
                        </label>
                        <label>
                                Claude Agent base URL
                                <input
                                        placeholder="https://agent.example.com"
                                        bind:value={voiceAgentUrl}
                                />
                        </label>
                        <label>
                                Claude Agent API key
                                <input
                                        type="password"
                                        bind:value={voiceAgentApiKey}
                                        autocomplete="off"
                                />
                        </label>
                        <label>
                                Agent ID (optional)
                                <input bind:value={voiceAgentId} />
                        </label>
                        <label>
                                Reply voice (optional)
                                <input bind:value={voiceAgentVoice} placeholder="studio" />
                        </label>
                </div>

                <LiveKitVoiceConsole
                        serverUrl={livekitServerUrl}
                        apiKey={livekitApiKey}
                        apiSecret={livekitApiSecret}
                        identity={livekitIdentity}
                        roomName={livekitRoom}
                        displayName={livekitName}
                        metadata={livekitMetadata}
                        ttlSeconds={livekitTtl}
                        canPublish={canPublish}
                        canSubscribe={canSubscribe}
                        canPublishData={canPublishData}
                        agentUrl={voiceAgentUrl}
                        agentApiKey={voiceAgentApiKey}
                        agentId={voiceAgentId}
                        agentVoice={voiceAgentVoice}
                />
        </section>

        <section class="panel">
                <header>
                        <h2>LiveKit Access Tokens</h2>
                        <p>
                                Generate short-lived LiveKit tokens for MPC or MCP powered voice debugging
                                sessions. These tokens can be handed to the Tauri interface or shared with a
                                paired ESP32 workflow.
                        </p>
                </header>

                <div class="grid">
                        <label>
                                API key
                                <input bind:value={livekitApiKey} />
                        </label>
                        <label>
                                API secret
                                <input type="password" bind:value={livekitApiSecret} />
                        </label>
                        <label>
                                Identity
                                <input bind:value={livekitIdentity} />
                        </label>
                        <label>
                                Room
                                <input bind:value={livekitRoom} />
                        </label>
                        <label>
                                Display name
                                <input bind:value={livekitName} />
                        </label>
                        <label>
                                Metadata
                                <input bind:value={livekitMetadata} />
                        </label>
                        <label>
                                TTL (seconds)
                                <input type="number" min="60" bind:value={livekitTtl} />
                        </label>
                </div>

                <div class="toggles">
                        <label><input type="checkbox" bind:checked={canPublish} /> Can publish</label>
                        <label><input type="checkbox" bind:checked={canSubscribe} /> Can subscribe</label>
                        <label><input type="checkbox" bind:checked={canPublishData} /> Can publish data</label>
                </div>

                <div class="actions">
                        <button
                                on:click={generateLivekitToken}
                                disabled={!livekitApiKey || !livekitApiSecret || !livekitIdentity || !livekitRoom}
                        >
                                Generate token
                        </button>
                </div>

                {#if $livekitError}
                        <p class="error">{$livekitError}</p>
                {/if}

                {#if $livekitToken}
                        <div class="token">
                                <header>
                                        <h3>Generated token</h3>
                                        <p>Expires at {$livekitExpiresAt}</p>
                                </header>
                                <textarea readonly rows="4">{$livekitToken}</textarea>
                        </div>
                {/if}
        </section>
</main>

<style>
        :global(body) {
                font-family: 'Inter', system-ui, sans-serif;
                margin: 0;
                background: #0f172a;
                color: #f8fafc;
        }

        main.page {
                display: flex;
                flex-direction: column;
                gap: 2.5rem;
                padding: 2.5rem;
        }

        section.panel {
                background: #111c34;
                border-radius: 16px;
                padding: 2rem;
                box-shadow: 0 20px 45px rgba(15, 23, 42, 0.45);
                border: 1px solid rgba(148, 163, 184, 0.1);
        }

        section.panel header > h1,
        section.panel header > h2 {
                margin: 0 0 0.5rem;
                font-weight: 600;
        }

        section.panel header > p {
                margin: 0 0 1.5rem;
                color: rgba(226, 232, 240, 0.85);
        }

        .grid {
                display: grid;
                gap: 1rem;
                grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
                margin-bottom: 1.5rem;
        }

        label {
                display: flex;
                flex-direction: column;
                gap: 0.5rem;
                font-size: 0.9rem;
                color: rgba(226, 232, 240, 0.9);
        }

        label input {
                padding: 0.75rem;
                border-radius: 10px;
                border: 1px solid rgba(148, 163, 184, 0.2);
                background: rgba(15, 23, 42, 0.65);
                color: inherit;
                font-size: 0.95rem;
        }

        .actions {
                display: flex;
                gap: 1rem;
                flex-wrap: wrap;
                margin-bottom: 1rem;
        }

        button {
                background: linear-gradient(135deg, #4f46e5, #22d3ee);
                border: none;
                color: #f8fafc;
                padding: 0.75rem 1.5rem;
                border-radius: 9999px;
                font-weight: 600;
                cursor: pointer;
                transition: transform 0.15s ease, box-shadow 0.15s ease;
        }

        button:disabled {
                opacity: 0.6;
                cursor: not-allowed;
                box-shadow: none;
                transform: none;
        }

        button:not(:disabled):hover {
                transform: translateY(-1px);
                box-shadow: 0 12px 24px rgba(79, 70, 229, 0.25);
        }

        .error {
                color: #f97316;
                font-weight: 600;
        }

        .metrics {
                display: flex;
                flex-direction: column;
                gap: 1.5rem;
        }

        .totals {
                display: grid;
                grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
                gap: 1rem;
        }

        .totals article {
                background: rgba(15, 23, 42, 0.55);
                border-radius: 12px;
                padding: 1rem;
                border: 1px solid rgba(79, 70, 229, 0.2);
        }

        .totals article h3 {
                margin: 0 0 0.5rem;
                font-size: 0.9rem;
                color: rgba(226, 232, 240, 0.8);
        }

        .totals article p {
                margin: 0;
                font-size: 1.3rem;
                font-weight: 600;
        }

        .details {
                display: grid;
                grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
                gap: 1rem;
        }

        .details ul {
                margin: 0;
                padding-left: 1rem;
                color: rgba(226, 232, 240, 0.85);
        }

        .token textarea {
                width: 100%;
                margin-top: 0.75rem;
                background: rgba(2, 6, 23, 0.8);
                color: #22d3ee;
                border-radius: 8px;
                border: 1px solid rgba(79, 70, 229, 0.4);
                padding: 1rem;
        }

        .token header h3 {
                margin: 0 0 0.5rem;
        }

        .toggles {
                display: flex;
                gap: 1.5rem;
                flex-wrap: wrap;
                margin-bottom: 1.25rem;
        }

        @media (max-width: 720px) {
                main.page {
                        padding: 1.25rem;
                }

                section.panel {
                        padding: 1.25rem;
                }
        }
</style>
