<script lang="ts" context="module">
        export const ssr = false
</script>

<script lang="ts">
        import { derived, get, writable } from 'svelte/store'
        import { onDestroy } from 'svelte'
        import { Room, RoomEvent, Track } from 'livekit-client'
        import { taurpc } from '$lib/tauri'
        import type { ClaudeMetricsSnapshot, ClaudeVoiceResponse } from '../types'

        const metrics = writable<ClaudeMetricsSnapshot | null>(null)
        const metricsLoading = writable(false)
        const metricsError = writable('')

        const voiceResponse = writable<ClaudeVoiceResponse | null>(null)
        const voiceError = writable('')
        const voiceLoading = writable(false)
        const voiceRecording = writable(false)
        const voiceAudioUrl = writable('')
        const livekitState = writable<'disconnected' | 'connecting' | 'connected'>('disconnected')
        const remoteParticipants = writable<Array<{ id: string; name: string }>>([])

        const livekitToken = writable('')
        const livekitError = writable('')
        const livekitExpiresAt = writable('')

        const voiceTranscript = derived(voiceResponse, ($voiceResponse) => $voiceResponse?.transcript ?? '')

        let dataDir = ''
        let hoursBack = 24
        let pythonPath = ''
        let serverUrl = 'http://localhost:8080'
        let authToken = ''

        let apiKey = ''
        let codeContext = ''
        let voiceModel = 'claude-3-5-sonnet-latest'
        let voiceMaxTokens = 800
        let voiceTemperature = 0.2
        let systemPrompt = 'You are a precise and concise code review assistant.'
        let voiceChoice = 'verse'
        let transcriptHint = ''
        let autoPlayVoice = true

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
        let livekitUrl = ''

        let voiceStatusMessage = ''

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

        let room: Room | null = null
        let recorder: MediaRecorder | null = null
        let recorderSourceStream: MediaStream | null = null
        let recordedChunks: BlobPart[] = []
        const remoteAudioElements = new Map<string, HTMLAudioElement>()
        let remoteAudioContainer: HTMLDivElement | null = null

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
                        const result = await taurpc[''].collect_claude_metrics({
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
                        const result = await taurpc[''].push_claude_metrics({
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

        async function generateLivekitToken() {
                livekitError.set('')
                livekitToken.set('')
                livekitExpiresAt.set('')
                try {
                        const result = await taurpc[''].generate_livekit_token({
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
                } catch (error) {
                        livekitError.set(error instanceof Error ? error.message : String(error))
                }
        }

        async function connectVoiceSession() {
                voiceError.set('')
                voiceStatusMessage = ''
                if (!livekitUrl.trim()) {
                        voiceError.set('Provide the LiveKit WebRTC URL (e.g., wss://example.livekit.cloud).')
                        return
                }
                if (!livekitApiKey.trim() || !livekitApiSecret.trim()) {
                        voiceError.set('LiveKit API key and secret are required to mint a session token.')
                        return
                }
                if (!livekitIdentity.trim() || !livekitRoom.trim()) {
                        voiceError.set('Identity and room are required for the LiveKit connection.')
                        return
                }
                livekitState.set('connecting')
                try {
                        const token = await taurpc[''].generate_livekit_token({
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

                        const instance = new Room({
                                adaptiveStream: true,
                                dynacast: true,
                                stopLocalTrackOnUnpublish: true,
                        })

                        instance.on(RoomEvent.TrackSubscribed, (track, publication, participant) => {
                                if (track.kind !== 'audio') return
                                const audioElement = track.attach()
                                audioElement.autoplay = true
                                audioElement.playsInline = true
                                if (remoteAudioContainer) {
                                        remoteAudioContainer.appendChild(audioElement)
                                } else {
                                        document.body.appendChild(audioElement)
                                }
                                remoteAudioElements.set(publication.sid, audioElement)
                                remoteParticipants.update((current) => {
                                        const next = current.filter((item) => item.id !== publication.sid)
                                        next.push({
                                                id: publication.sid,
                                                name: participant.name || participant.identity,
                                        })
                                        return next
                                })
                        })

                        instance.on(RoomEvent.TrackUnsubscribed, (_track, publication) => {
                                const audio = remoteAudioElements.get(publication.sid)
                                if (audio) {
                                        audio.srcObject = null
                                        audio.remove()
                                        remoteAudioElements.delete(publication.sid)
                                }
                                remoteParticipants.update((current) =>
                                        current.filter((item) => item.id !== publication.sid),
                                )
                        })

                        instance.on(RoomEvent.ParticipantDisconnected, (participant) => {
                                remoteParticipants.update((current) =>
                                        current.filter((item) => item.name !== (participant.name || participant.identity)),
                                )
                        })

                        instance.on(RoomEvent.Disconnected, () => {
                                cleanupRoom()
                                livekitState.set('disconnected')
                        })

                        await instance.connect(livekitUrl.trim(), token.token)
                        await instance.localParticipant.setMicrophoneEnabled(true)
                        room = instance
                        livekitState.set('connected')
                        voiceStatusMessage = 'Connected to LiveKit. Hold “Record Question” to dictate your prompt.'
                } catch (error) {
                        cleanupRoom()
                        livekitState.set('disconnected')
                        voiceError.set(error instanceof Error ? error.message : String(error))
                }
        }

        async function disconnectVoiceSession() {
                voiceStatusMessage = ''
                if (recorder) {
                        recorder.stop()
                        recorder = null
                }
                if (recorderSourceStream) {
                        recorderSourceStream.getTracks().forEach((track) => track.stop())
                        recorderSourceStream = null
                }
                if (room) {
                        room.disconnect()
                        room = null
                }
                cleanupRoom()
                livekitState.set('disconnected')
        }

        function cleanupRoom() {
                remoteParticipants.set([])
                remoteAudioElements.forEach((audio) => {
                        audio.srcObject = null
                        audio.remove()
                })
                remoteAudioElements.clear()
        }

        async function startRecording() {
                voiceError.set('')
                voiceStatusMessage = ''
                voiceResponse.set(null)
                voiceAudioUrl.set('')
                if (!apiKey.trim()) {
                        voiceError.set('Claude API key is required before capturing audio.')
                        return
                }
                if (get(livekitState) !== 'connected') {
                        voiceError.set('Connect to LiveKit before recording a question.')
                        return
                }

                const stream = await getRecordingStream()
                if (!stream) {
                        if (!get(voiceError)) {
                                voiceError.set('Unable to access microphone audio for recording.')
                        }
                        return
                }

                recordedChunks = []
                let localRecorder: MediaRecorder | null = null
                try {
                        localRecorder = new MediaRecorder(stream, {
                                mimeType: 'audio/webm;codecs=opus',
                        })
                } catch {
                        try {
                                localRecorder = new MediaRecorder(stream)
                        } catch (error) {
                                voiceError.set(
                                        error instanceof Error
                                                ? error.message
                                                : 'The browser does not support MediaRecorder for this audio format.',
                                )
                                stream.getTracks().forEach((track) => track.stop())
                                return
                        }
                }

                recorder = localRecorder
                recorderSourceStream = stream
                const mimeType = localRecorder.mimeType || 'audio/webm'
                localRecorder.ondataavailable = (event) => {
                        if (event.data && event.data.size > 0) {
                                recordedChunks.push(event.data)
                        }
                }
                localRecorder.onerror = (event) => {
                        voiceError.set(event.error?.message ?? 'An error occurred while recording audio.')
                        stopRecording()
                }
                localRecorder.onstop = async () => {
                        const chunks = recordedChunks.slice()
                        recordedChunks = []
                        recorder = null
                        voiceRecording.set(false)
                        if (recorderSourceStream) {
                                recorderSourceStream.getTracks().forEach((track) => track.stop())
                                recorderSourceStream = null
                        }
                        if (!chunks.length) {
                                voiceError.set('No audio captured from the microphone.')
                                return
                        }
                        try {
                                const blob = new Blob(chunks, { type: mimeType })
                                voiceStatusMessage = 'Sending your recording to Claude…'
                                await submitVoiceSample(blob, mimeType)
                        } catch (error) {
                                voiceError.set(error instanceof Error ? error.message : String(error))
                        }
                }

                voiceRecording.set(true)
                voiceStatusMessage = 'Recording… release when you finish your question.'
                localRecorder.start()
        }

        async function stopRecording() {
                if (recorder && recorder.state !== 'inactive') {
                        recorder.stop()
                }
        }

        async function submitVoiceSample(blob: Blob, mimeType: string) {
                voiceLoading.set(true)
                voiceError.set('')
                try {
                        const base64 = await blobToBase64(blob)
                        const result = await taurpc[''].ask_claude_voice({
                                api_key: apiKey,
                                audio_base64: base64,
                                audio_format: audioMimeToFormat(mimeType),
                                transcript_hint: transcriptHint || null,
                                code_context: codeContext || null,
                                model: voiceModel || null,
                                max_output_tokens: Number.isFinite(voiceMaxTokens) ? voiceMaxTokens : null,
                                temperature: Number.isFinite(voiceTemperature) ? voiceTemperature : null,
                                system_prompt: systemPrompt || null,
                                voice: voiceChoice || null,
                        })
                        voiceResponse.set(result)
                        voiceStatusMessage = 'Claude responded. Review the transcript and answer below.'
                        if (result.answer_audio_base64 && result.answer_audio_mime_type) {
                                const url = `data:${result.answer_audio_mime_type};base64,${result.answer_audio_base64}`
                                voiceAudioUrl.set(url)
                                if (autoPlayVoice) {
                                        try {
                                                const audio = new Audio(url)
                                                audio.play().catch(() => {
                                                        /* ignore autoplay restrictions */
                                                })
                                        } catch {
                                                /* ignore playback errors */
                                        }
                                }
                        } else {
                                voiceAudioUrl.set('')
                        }
                } catch (error) {
                        voiceError.set(error instanceof Error ? error.message : String(error))
                } finally {
                        voiceLoading.set(false)
                }
        }

        async function getRecordingStream(): Promise<MediaStream | null> {
                if (room) {
                        const publication = room.localParticipant.audioTrackPublications.find((pub) =>
                                pub.source === Track.Source.Microphone && pub.track,
                        )
                        if (publication?.track) {
                                const cloned = publication.track.mediaStreamTrack.clone()
                                return new MediaStream([cloned])
                        }
                }
                try {
                        return await navigator.mediaDevices.getUserMedia({ audio: true })
                } catch (error) {
                        voiceError.set(error instanceof Error ? error.message : String(error))
                        return null
                }
        }

        async function blobToBase64(blob: Blob) {
                const buffer = await blob.arrayBuffer()
                const bytes = new Uint8Array(buffer)
                let binary = ''
                const chunkSize = 0x8000
                for (let i = 0; i < bytes.length; i += chunkSize) {
                        const chunk = bytes.subarray(i, i + chunkSize)
                        binary += String.fromCharCode(...chunk)
                }
                return btoa(binary)
        }

        function audioMimeToFormat(mimeType: string) {
                if (!mimeType.includes('/')) return mimeType || 'webm'
                const [, subtypeRaw] = mimeType.split('/')
                if (!subtypeRaw) return 'webm'
                const subtype = subtypeRaw.split(';')[0]?.trim().toLowerCase()
                if (!subtype) return 'webm'

                switch (subtype) {
                        case 'wav':
                        case 'wave':
                        case 'x-wav':
                        case 'vnd.wave':
                                return 'wav'
                        case 'mp3':
                        case 'mpeg':
                                return 'mp3'
                        case 'ogg':
                        case 'oga':
                        case 'x-ogg':
                                return 'ogg'
                        case 'webm':
                                return 'webm'
                        default:
                                return subtype
                }
        }

        onDestroy(() => {
                if (recorder && recorder.state !== 'inactive') {
                        recorder.stop()
                }
                if (recorderSourceStream) {
                        recorderSourceStream.getTracks().forEach((track) => track.stop())
                        recorderSourceStream = null
                }
                if (room) {
                        room.disconnect()
                        room = null
                }
                cleanupRoom()
        })
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
                        <button class="primary" on:click={loadMetrics} disabled={$metricsLoading}>
                                {#if $metricsLoading}
                                        Collecting…
                                {:else}
                                        Collect metrics
                                {/if}
                        </button>
                        <button class="secondary" on:click={syncMetrics} disabled={$metricsLoading}>
                                Push latest snapshot
                        </button>
                </div>

                {#if $metricsError}
                        <p class="error">{$metricsError}</p>
                {/if}

                {#if $metrics}
                        <section class="snapshot">
                                <h2>Latest snapshot</h2>
                                <dl>
                                        {#if $metricsTotals}
                                                {#each $metricsTotals as item}
                                                        <div>
                                                                <dt>{item.label}</dt>
                                                                <dd>{item.value}</dd>
                                                        </div>
                                                {/each}
                                        {/if}
                                        <div>
                                                <dt>Input tokens</dt>
                                                <dd>{formatNumber($metrics.input_tokens)}</dd>
                                        </div>
                                        <div>
                                                <dt>Output tokens</dt>
                                                <dd>{formatNumber($metrics.output_tokens)}</dd>
                                        </div>
                                        <div>
                                                <dt>Cache creation tokens</dt>
                                                <dd>{formatNumber($metrics.cache_creation_tokens)}</dd>
                                        </div>
                                        <div>
                                                <dt>Cache read tokens</dt>
                                                <dd>{formatNumber($metrics.cache_read_tokens)}</dd>
                                        </div>
                                        <div>
                                                <dt>Last activity</dt>
                                                <dd>{new Date($metrics.last_activity).toLocaleString()}</dd>
                                        </div>
                                        <div>
                                                <dt>Source</dt>
                                                <dd>{$metrics.source ?? 'Desktop monitor'}</dd>
                                        </div>
                                </dl>
                        </section>
                {/if}
        </section>

        <section class="panel">
                <div class="sr-only" aria-hidden="true" bind:this={remoteAudioContainer}></div>
                <header>
                        <h1>LiveKit Voice Debugging</h1>
                        <p>
                                Connect to LiveKit, dictate your questions, and let Claude reply with both
                                text and synthesized speech. Use the same session to collaborate with teammates
                                or stream the assistant’s audio back to the ESP32 workflow.
                        </p>
                </header>

                <div class="voice-grid">
                        <label>
                                LiveKit URL
                                <input
                                        placeholder="wss://example.livekit.cloud"
                                        bind:value={livekitUrl}
                                />
                        </label>
                        <label>
                                LiveKit API key
                                <input
                                        placeholder="LKXXXXXXXXXXXXXXXX"
                                        bind:value={livekitApiKey}
                                />
                        </label>
                        <label>
                                LiveKit API secret
                                <input type="password" bind:value={livekitApiSecret} />
                        </label>
                        <label>
                                Participant identity
                                <input
                                        placeholder="laptop-debugger"
                                        bind:value={livekitIdentity}
                                />
                        </label>
                        <label>
                                Room name
                                <input placeholder="debug-room" bind:value={livekitRoom} />
                        </label>
                        <label>
                                Display name (optional)
                                <input bind:value={livekitName} />
                        </label>
                        <label>
                                Metadata (optional)
                                <input bind:value={livekitMetadata} />
                        </label>
                        <label>
                                Token TTL (seconds)
                                <input type="number" min="60" bind:value={livekitTtl} />
                        </label>
                        <fieldset>
                                <legend>LiveKit permissions</legend>
                                <label class="checkbox">
                                        <input type="checkbox" bind:checked={canPublish} />
                                        <span>Publish tracks</span>
                                </label>
                                <label class="checkbox">
                                        <input type="checkbox" bind:checked={canSubscribe} />
                                        <span>Subscribe to tracks</span>
                                </label>
                                <label class="checkbox">
                                        <input type="checkbox" bind:checked={canPublishData} />
                                        <span>Publish data</span>
                                </label>
                        </fieldset>
                        <label>
                                Claude API key
                                <input type="password" bind:value={apiKey} />
                        </label>
                        <label>
                                Claude model
                                <input bind:value={voiceModel} />
                        </label>
                        <label>
                                Max output tokens
                                <input type="number" min="1" bind:value={voiceMaxTokens} />
                        </label>
                        <label>
                                Temperature
                                <input type="number" step="0.1" bind:value={voiceTemperature} />
                        </label>
                        <label>
                                Voice
                                <input bind:value={voiceChoice} />
                        </label>
                        <label class="checkbox">
                                <input type="checkbox" bind:checked={autoPlayVoice} />
                                <span>Auto play Claude’s reply</span>
                        </label>
                </div>

                <label class="stacked">
                        System prompt
                        <textarea rows="3" bind:value={systemPrompt} />
                </label>
                <label class="stacked">
                        Code context (optional)
                        <textarea rows="3" bind:value={codeContext} />
                </label>
                <label class="stacked">
                        Transcript hint (optional)
                        <textarea rows="2" bind:value={transcriptHint} />
                </label>

                <div class="voice-actions">
                        {#if $livekitState === 'connected'}
                                <button class="secondary" on:click={disconnectVoiceSession}>
                                        Disconnect
                                </button>
                        {:else}
                                <button
                                        class="primary"
                                        on:click={connectVoiceSession}
                                        disabled={$livekitState === 'connecting'}
                                >
                                        {#if $livekitState === 'connecting'}
                                                Connecting…
                                        {:else}
                                                Connect & enable mic
                                        {/if}
                                </button>
                        {/if}
                        <button
                                class={`record ${$voiceRecording ? 'recording' : ''}`}
                                on:click={$voiceRecording ? stopRecording : startRecording}
                                disabled={$livekitState !== 'connected' || $voiceLoading}
                        >
                                {$voiceRecording ? 'Stop recording' : 'Record question'}
                        </button>
                </div>

                {#if voiceStatusMessage}
                        <p class="status">{voiceStatusMessage}</p>
                {/if}

                {#if $voiceError}
                        <p class="error">{$voiceError}</p>
                {/if}

                {#if $voiceLoading}
                        <p class="status">Claude is processing your audio…</p>
                {/if}

                {#if $remoteParticipants.length}
                        <section class="remotes">
                                <h2>Remote participants</h2>
                                <ul>
                                        {#each $remoteParticipants as participant}
                                                <li>{participant.name}</li>
                                        {/each}
                                </ul>
                        </section>
                {/if}

                {#if $voiceResponse}
                        <section class="voice-result">
                                {#if $voiceTranscript}
                                        <div>
                                                <h2>Transcript</h2>
                                                <p>{$voiceTranscript}</p>
                                        </div>
                                {/if}
                                <div>
                                        <h2>Claude’s answer</h2>
                                        <p>{$voiceResponse.answer_text}</p>
                                </div>
                                <dl>
                                        <div>
                                                <dt>Model</dt>
                                                <dd>{$voiceResponse.model}</dd>
                                        </div>
                                        {#if $voiceResponse.stop_reason}
                                                <div>
                                                        <dt>Stop reason</dt>
                                                        <dd>{$voiceResponse.stop_reason}</dd>
                                                </div>
                                        {/if}
                                        {#if $voiceResponse.usage}
                                                <div>
                                                        <dt>Tokens</dt>
                                                        <dd>
                                                                In: {$voiceResponse.usage.input_tokens} · Out:
                                                                {$voiceResponse.usage.output_tokens}
                                                        </dd>
                                                </div>
                                        {/if}
                                </dl>
                        </section>
                {/if}

                {#if $voiceAudioUrl}
                        <section class="voice-audio">
                                <h2>Claude’s voice reply</h2>
                                <audio controls src={$voiceAudioUrl}></audio>
                        </section>
                {/if}
        </section>

        <section class="panel">
                <header>
                        <h1>Standalone LiveKit token</h1>
                        <p>
                                Need to onboard another device quickly? Mint a scoped token with the same
                                credentials you use for the voice session.
                        </p>
                </header>

                <div class="actions">
                        <button class="secondary" on:click={generateLivekitToken}>
                                Generate token
                        </button>
                </div>

                {#if $livekitError}
                        <p class="error">{$livekitError}</p>
                {/if}

                {#if $livekitToken}
                        <section class="snapshot">
                                <h2>Latest token</h2>
                                <dl>
                                        <div>
                                                <dt>Token</dt>
                                                <dd class="token">{$livekitToken}</dd>
                                        </div>
                                        <div>
                                                <dt>Expires at</dt>
                                                <dd>{$livekitExpiresAt}</dd>
                                        </div>
                                </dl>
                        </section>
                {/if}
        </section>
</main>

<style>
        :global(body) {
                background: #0f1115;
                color: #f8fafc;
                font-family: 'Inter', system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
                margin: 0;
        }

        .sr-only {
                position: absolute;
                width: 1px;
                height: 1px;
                padding: 0;
                margin: -1px;
                overflow: hidden;
                clip: rect(0, 0, 0, 0);
                white-space: nowrap;
                border: 0;
        }

        main.page {
                display: grid;
                gap: 1.5rem;
                padding: 2rem;
                max-width: 1200px;
                margin: 0 auto;
        }

        .panel {
                background: linear-gradient(145deg, rgba(32, 35, 45, 0.95), rgba(22, 24, 32, 0.95));
                border: 1px solid rgba(148, 163, 184, 0.12);
                border-radius: 1rem;
                padding: 1.5rem;
                box-shadow: 0 25px 45px rgba(15, 17, 21, 0.4);
                display: flex;
                flex-direction: column;
                gap: 1.25rem;
        }

        header h1 {
                font-size: 1.5rem;
                margin: 0 0 0.25rem;
        }

        header p {
                margin: 0;
                color: rgba(226, 232, 240, 0.75);
                line-height: 1.4;
        }

        .grid,
        .voice-grid {
                display: grid;
                gap: 1rem;
        }

        .grid {
                grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
        }

        .voice-grid {
                grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
        }

        label {
                display: flex;
                flex-direction: column;
                gap: 0.4rem;
                font-size: 0.9rem;
        }

        label input,
        label textarea {
                background: rgba(15, 23, 42, 0.55);
                border: 1px solid rgba(148, 163, 184, 0.25);
                border-radius: 0.6rem;
                padding: 0.65rem 0.75rem;
                color: #f1f5f9;
                font-size: 0.95rem;
                outline: none;
                transition: border 0.2s ease, box-shadow 0.2s ease;
        }

        label input:focus,
        label textarea:focus {
                border-color: rgba(56, 189, 248, 0.75);
                box-shadow: 0 0 0 2px rgba(56, 189, 248, 0.25);
        }

        .checkbox {
                flex-direction: row;
                align-items: center;
                gap: 0.5rem;
        }

        .checkbox input {
                width: 1rem;
                height: 1rem;
                accent-color: #38bdf8;
        }

        fieldset {
                border: 1px solid rgba(148, 163, 184, 0.25);
                border-radius: 0.8rem;
                padding: 0.75rem 1rem 0.5rem;
                display: flex;
                flex-direction: column;
                gap: 0.5rem;
                min-width: 0;
        }

        fieldset legend {
                padding: 0 0.35rem;
                color: rgba(226, 232, 240, 0.8);
        }

        .stacked textarea {
                min-height: 5.5rem;
        }

        .actions,
        .voice-actions {
                display: flex;
                flex-wrap: wrap;
                gap: 0.75rem;
        }

        button {
                border: none;
                border-radius: 999px;
                padding: 0.65rem 1.3rem;
                font-size: 0.95rem;
                cursor: pointer;
                transition: transform 0.2s ease, box-shadow 0.2s ease;
                font-weight: 600;
        }

        button:disabled {
                cursor: not-allowed;
                opacity: 0.6;
        }

        button.primary {
                background: linear-gradient(120deg, #38bdf8, #818cf8);
                color: #0f172a;
                box-shadow: 0 10px 25px rgba(129, 140, 248, 0.35);
        }

        button.secondary {
                background: rgba(148, 163, 184, 0.15);
                color: #e2e8f0;
        }

        button.primary:not(:disabled):hover,
        button.secondary:not(:disabled):hover,
        .record:not(:disabled):hover {
                transform: translateY(-1px);
        }

        .record {
                background: rgba(248, 113, 113, 0.15);
                color: #fecaca;
                border: 1px solid rgba(248, 113, 113, 0.35);
        }

        .record.recording {
                background: linear-gradient(120deg, #f87171, #ef4444);
                color: #0b1120;
                box-shadow: 0 12px 30px rgba(248, 113, 113, 0.4);
        }

        .error {
                color: #fca5a5;
                margin: 0;
        }

        .status {
                color: rgba(148, 197, 255, 0.85);
                margin: 0;
        }

        .snapshot dl {
                display: grid;
                gap: 0.75rem;
                grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
                margin: 1rem 0 0;
        }

        .snapshot dt {
                color: rgba(148, 163, 184, 0.75);
                font-size: 0.85rem;
        }

        .snapshot dd {
                margin: 0.15rem 0 0;
                font-size: 1.1rem;
                font-weight: 600;
        }

        .snapshot .token {
                word-break: break-all;
                font-family: 'JetBrains Mono', 'SFMono-Regular', Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New',
                        monospace;
                font-size: 0.85rem;
                line-height: 1.4;
        }

        .remotes ul {
                list-style: none;
                padding: 0;
                margin: 0;
                display: flex;
                flex-wrap: wrap;
                gap: 0.5rem;
        }

        .remotes li {
                padding: 0.4rem 0.75rem;
                background: rgba(148, 163, 184, 0.15);
                border-radius: 999px;
                font-size: 0.85rem;
        }

        .voice-result {
                display: grid;
                gap: 1rem;
        }

        .voice-result h2,
        .voice-audio h2 {
                margin: 0 0 0.25rem;
        }

        .voice-result p {
                background: rgba(15, 23, 42, 0.6);
                border-radius: 0.75rem;
                padding: 0.75rem 1rem;
                line-height: 1.5;
                white-space: pre-wrap;
        }

        .voice-result dl {
                display: grid;
                grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
                gap: 0.75rem;
        }

        audio {
                width: 100%;
                border-radius: 0.75rem;
        }

        @media (min-width: 1024px) {
                main.page {
                        grid-template-columns: repeat(2, minmax(0, 1fr));
                }

                main.page > .panel:nth-child(3) {
                        grid-column: span 2;
                }
        }
</style>
