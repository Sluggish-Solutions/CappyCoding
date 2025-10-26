<script lang="ts">
        import { onDestroy, onMount } from 'svelte'
        import { derived, get, writable } from 'svelte/store'
        import { Room, RoomEvent, Track, type RemoteAudioTrack } from 'livekit-client'
        import { taurpc } from '$lib/tauri'
        import type {
                ClaudeMetricsSnapshot,
                ClaudeVoicePromptResponse,
                LivekitTokenResponse,
        } from '../types'

        const metrics = writable<ClaudeMetricsSnapshot | null>(null)
        const metricsLoading = writable(false)
        const metricsError = writable('')

        const livekitToken = writable('')
        const livekitError = writable('')
        const livekitExpiresAt = writable('')

        const voiceConnectionState = writable<'idle' | 'connecting' | 'connected'>('idle')
        const voiceSessionError = writable('')
        const voiceLoading = writable(false)
        const voiceError = writable('')
        const voiceTranscript = writable('')
        const voiceReplyText = writable('')
        const voiceReplyAudioUrl = writable('')
        const recordedAudioUrl = writable('')
        const voiceSessionId = writable('')

        const remoteAudioTracks = writable<
                { id: string; participant: string; identity: string; track: RemoteAudioTrack }[]
        >([])

        const isRecording = writable(false)
        const recordingSupported = writable(false)

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

        let room: Room | null = null
        let recorder: MediaRecorder | null = null
        let recorderStream: MediaStream | null = null
        let recorderChunks: Blob[] = []

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

        async function connectVoiceSession() {
                voiceSessionError.set('')
                if (get(voiceConnectionState) === 'connecting') {
                        return
                }
                if (!livekitServerUrl.trim()) {
                        voiceSessionError.set('Provide the LiveKit server URL before connecting.')
                        return
                }
                if (
                        !livekitApiKey.trim() ||
                        !livekitApiSecret.trim() ||
                        !livekitIdentity.trim() ||
                        !livekitRoom.trim()
                ) {
                        voiceSessionError.set(
                                'LiveKit API key, secret, identity, and room are required to join the session.',
                        )
                        return
                }

                voiceConnectionState.set('connecting')
                try {
                        const tokenResult = await generateLivekitToken()
                        if (!tokenResult) {
                                throw new Error(get(livekitError) || 'Failed to create a LiveKit token.')
                        }
                        await establishRoomConnection(livekitServerUrl, tokenResult.token)
                } catch (error) {
                        voiceSessionError.set(error instanceof Error ? error.message : String(error))
                        await disconnectRoomSilently()
                        voiceConnectionState.set('idle')
                }
        }

        async function disconnectVoiceSession() {
                voiceSessionError.set('')
                await disconnectRoomSilently()
                voiceConnectionState.set('idle')
        }

        async function establishRoomConnection(serverUrl: string, token: string) {
                await disconnectRoomSilently()

                const nextRoom = new Room()
                registerRoomListeners(nextRoom)
                room = nextRoom

                try {
                        await nextRoom.connect(serverUrl, token)
                        await nextRoom.localParticipant.setMicrophoneEnabled(true)
                        voiceConnectionState.set('connected')
                        voiceSessionError.set('')
                } catch (error) {
                        nextRoom.removeAllListeners()
                        room = null
                        clearRemoteAudioTracks()
                        throw error
                }
        }

        async function disconnectRoomSilently() {
                if (!room) return
                try {
                        await room.localParticipant.setMicrophoneEnabled(false)
                } catch (error) {
                        console.error('failed to disable microphone', error)
                }
                try {
                        await room.disconnect()
                } catch (error) {
                        console.error('failed to disconnect LiveKit room', error)
                }
                cleanupRoom()
        }

        function registerRoomListeners(activeRoom: Room) {
                activeRoom.on(RoomEvent.ConnectionStateChanged, (state) => {
                        if (state === 'connected') {
                                voiceConnectionState.set('connected')
                                voiceSessionError.set('')
                        } else if (state === 'disconnected') {
                                voiceConnectionState.set('idle')
                                clearRemoteAudioTracks()
                        } else if (state === 'connecting') {
                                voiceConnectionState.set('connecting')
                        }
                })

                activeRoom.on(RoomEvent.Disconnected, () => {
                        voiceConnectionState.set('idle')
                        cleanupRoom()
                })

                activeRoom.on(RoomEvent.TrackSubscribed, (track, publication, participant) => {
                        if (track.kind === Track.Kind.Audio) {
                                const publicationSid =
                                        publication.trackSid ??
                                        (track as RemoteAudioTrack).sid ??
                                        `${participant.identity}-${Math.random().toString(36).slice(2)}`
                                addRemoteAudioTrackEntry(
                                        track as RemoteAudioTrack,
                                        publicationSid,
                                        participant.identity,
                                        participant.name ?? participant.identity,
                                )
                        }
                })

                activeRoom.on(RoomEvent.TrackUnsubscribed, (_, publication, participant) => {
                        const publicationSid = publication.trackSid
                        if (publicationSid) {
                                removeRemoteAudioTrackEntry(publicationSid, participant.identity)
                        } else {
                                removeParticipantAudioTracks(participant.identity)
                        }
                })

                activeRoom.on(RoomEvent.ParticipantDisconnected, (participant) => {
                        removeParticipantAudioTracks(participant.identity)
                })
        }

        function addRemoteAudioTrackEntry(
                track: RemoteAudioTrack,
                publicationSid: string,
                identity: string,
                displayName: string,
        ) {
                remoteAudioTracks.update((tracks) => {
                        if (tracks.some((entry) => entry.id === publicationSid)) {
                                return tracks.map((entry) =>
                                        entry.id === publicationSid
                                                ? { ...entry, track, participant: displayName, identity }
                                                : entry,
                                )
                        }
                        return [
                                ...tracks,
                                {
                                        id: publicationSid,
                                        participant: displayName,
                                        identity,
                                        track,
                                },
                        ]
                })
        }

        function removeRemoteAudioTrackEntry(publicationSid: string, identity: string) {
                remoteAudioTracks.update((tracks) => {
                        return tracks.filter((entry) => {
                                const matchesPublication = publicationSid && entry.id === publicationSid
                                const matchesIdentity = !publicationSid && entry.identity === identity
                                if (matchesPublication || matchesIdentity) {
                                        entry.track.detach()
                                        return false
                                }
                                return true
                        })
                })
        }

        function removeParticipantAudioTracks(identity: string) {
                remoteAudioTracks.update((tracks) => {
                        tracks
                                .filter((entry) => entry.identity === identity)
                                .forEach((entry) => entry.track.detach())
                        return tracks.filter((entry) => entry.identity !== identity)
                })
        }

        function clearRemoteAudioTracks() {
                remoteAudioTracks.update((tracks) => {
                        tracks.forEach((entry) => entry.track.detach())
                        return []
                })
        }

        function cleanupRoom() {
                clearRemoteAudioTracks()
                if (room) {
                        room.removeAllListeners()
                        room = null
                }
        }

        function attachAudio(node: HTMLAudioElement, track: RemoteAudioTrack | null) {
                let currentTrack: RemoteAudioTrack | null = track
                if (currentTrack) {
                        currentTrack.attach(node)
                }
                node.autoplay = true
                node.setAttribute('playsinline', 'true')
                return {
                        update(newTrack: RemoteAudioTrack | null) {
                                if (currentTrack && currentTrack !== newTrack) {
                                        currentTrack.detach(node)
                                }
                                currentTrack = newTrack
                                if (currentTrack) {
                                        currentTrack.attach(node)
                                }
                        },
                        destroy() {
                                if (currentTrack) {
                                        currentTrack.detach(node)
                                }
                        },
                }
        }

        onMount(() => {
                let supported = false
                try {
                        supported =
                                typeof navigator !== 'undefined' &&
                                typeof navigator.mediaDevices !== 'undefined' &&
                                typeof navigator.mediaDevices.getUserMedia === 'function' &&
                                typeof window !== 'undefined' &&
                                typeof (window as typeof window & { MediaRecorder?: typeof MediaRecorder })
                                        .MediaRecorder !== 'undefined'
                } catch (error) {
                        console.warn('MediaRecorder detection failed', error)
                }
                recordingSupported.set(supported)

                return () => {
                        void disconnectRoomSilently()
                        if (recorderStream) {
                                recorderStream.getTracks().forEach((track) => track.stop())
                                recorderStream = null
                        }
                        const previewUrl = get(recordedAudioUrl)
                        if (previewUrl) {
                                URL.revokeObjectURL(previewUrl)
                        }
                        const replyUrl = get(voiceReplyAudioUrl)
                        if (replyUrl) {
                                URL.revokeObjectURL(replyUrl)
                        }
                }
        })

        onDestroy(() => {
                if (recorderStream) {
                        recorderStream.getTracks().forEach((track) => track.stop())
                        recorderStream = null
                }
                void disconnectRoomSilently()
                const previewUrl = get(recordedAudioUrl)
                if (previewUrl) {
                        URL.revokeObjectURL(previewUrl)
                }
                const replyUrl = get(voiceReplyAudioUrl)
                if (replyUrl) {
                        URL.revokeObjectURL(replyUrl)
                }
        })

        async function startVoiceRecording() {
                voiceError.set('')
                if (get(isRecording)) {
                        return
                }
                if (get(voiceLoading)) {
                        voiceError.set('Wait for the previous prompt to finish processing.')
                        return
                }
                if (get(voiceConnectionState) !== 'connected') {
                        voiceError.set('Join the LiveKit room before recording a prompt.')
                        return
                }
                if (!voiceAgentApiKey.trim() || !voiceAgentUrl.trim()) {
                        voiceError.set('Provide the Claude Agent base URL and API key to continue.')
                        return
                }
                try {
                        recorderStream = await navigator.mediaDevices.getUserMedia({ audio: true })
                        recorderChunks = []
                        const options: MediaRecorderOptions = {}
                        if (
                                typeof MediaRecorder !== 'undefined' &&
                                MediaRecorder.isTypeSupported('audio/webm')
                        ) {
                                options.mimeType = 'audio/webm'
                        }
                        recorder = new MediaRecorder(recorderStream, options)
                        const mimeType = recorder.mimeType || 'audio/webm'
                        recorder.ondataavailable = (event) => {
                                if (event.data.size > 0) {
                                        recorderChunks.push(event.data)
                                }
                        }
                        recorder.onerror = (event) => {
                                const mediaError = event as MediaRecorderErrorEvent
                                voiceError.set(mediaError?.error?.message ?? 'Recording error')
                        }
                        recorder.onstop = async () => {
                                isRecording.set(false)
                                if (recorderStream) {
                                        recorderStream.getTracks().forEach((track) => track.stop())
                                        recorderStream = null
                                }
                                const previousPreview = get(recordedAudioUrl)
                                if (previousPreview) {
                                        URL.revokeObjectURL(previousPreview)
                                }
                                if (recorderChunks.length === 0) {
                                        voiceError.set('No audio captured from the microphone.')
                                        recorder = null
                                        return
                                }
                                const blob = new Blob(recorderChunks, { type: mimeType })
                                const previewUrl = URL.createObjectURL(blob)
                                recordedAudioUrl.set(previewUrl)
                                recorderChunks = []
                                recorder = null
                                await processVoicePrompt(blob, mimeType)
                        }
                        recorder.start()
                        isRecording.set(true)
                } catch (error) {
                        voiceError.set(error instanceof Error ? error.message : String(error))
                        if (recorderStream) {
                                recorderStream.getTracks().forEach((track) => track.stop())
                                recorderStream = null
                        }
                        recorder = null
                }
        }

        function stopVoiceRecording() {
                if (!recorder) return
                if (recorder.state !== 'inactive') {
                        recorder.stop()
                }
        }

        async function processVoicePrompt(blob: Blob, mimeType: string) {
                voiceLoading.set(true)
                voiceError.set('')
                try {
                        const audioBase64 = await blobToBase64(blob)
                        const response = await rpc.relay_claude_voice({
                                api_key: voiceAgentApiKey,
                                agent_url: voiceAgentUrl,
                                audio_base64: audioBase64,
                                mime_type: mimeType,
                                agent_id: voiceAgentId || null,
                                response_voice: voiceAgentVoice || null,
                                session_id: get(voiceSessionId) || null,
                        })
                        handleVoiceResponse(response)
                } catch (error) {
                        voiceError.set(error instanceof Error ? error.message : String(error))
                } finally {
                        voiceLoading.set(false)
                }
        }

        function handleVoiceResponse(response: ClaudeVoicePromptResponse) {
                voiceTranscript.set(response.transcript || '')
                voiceReplyText.set(response.reply_text || '')
                voiceSessionId.set(response.session_id ?? '')
                const previousReply = get(voiceReplyAudioUrl)
                if (previousReply) {
                        URL.revokeObjectURL(previousReply)
                }
                if (response.reply_audio_base64 && response.reply_audio_mime_type) {
                        const replyBlob = base64ToBlob(
                                response.reply_audio_base64,
                                response.reply_audio_mime_type,
                        )
                        voiceReplyAudioUrl.set(URL.createObjectURL(replyBlob))
                } else {
                        voiceReplyAudioUrl.set('')
                }
        }

        function blobToBase64(blob: Blob): Promise<string> {
                return new Promise((resolve, reject) => {
                        const reader = new FileReader()
                        reader.onloadend = () => {
                                const result = reader.result
                                if (typeof result === 'string') {
                                        const [, base64] = result.split(',', 2)
                                        resolve(base64 ?? '')
                                } else {
                                        reject(new Error('Failed to encode audio to base64'))
                                }
                        }
                        reader.onerror = () => reject(reader.error ?? new Error('Failed to read audio blob'))
                        reader.readAsDataURL(blob)
                })
        }

        function base64ToBlob(base64: string, mimeType: string) {
                const binary = atob(base64)
                const length = binary.length
                const bytes = new Uint8Array(length)
                for (let i = 0; i < length; i += 1) {
                        bytes[i] = binary.charCodeAt(i)
                }
                return new Blob([bytes], { type: mimeType })
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

                <div class="voice-session-meta">
                        <div>
                                <span class="status-label">Connection</span>
                                <strong>{$voiceConnectionState}</strong>
                        </div>
                        {#if $voiceSessionId}
                                <div>
                                        <span class="status-label">Session</span>
                                        <code>{$voiceSessionId}</code>
                                </div>
                        {/if}
                </div>

                <div class="actions">
                        <button
                                on:click={$voiceConnectionState === 'connected'
                                        ? disconnectVoiceSession
                                        : connectVoiceSession}
                                disabled={$voiceConnectionState === 'connecting'}
                        >
                                {#if $voiceConnectionState === 'connected'}
                                        Disconnect
                                {:else if $voiceConnectionState === 'connecting'}
                                        Connecting…
                                {:else}
                                        Join LiveKit room
                                {/if}
                        </button>
                        <button
                                on:click={$isRecording ? stopVoiceRecording : startVoiceRecording}
                                disabled={$voiceConnectionState !== 'connected' || !$recordingSupported || $voiceLoading}
                        >
                                {#if $voiceLoading}
                                        Processing…
                                {:else if $isRecording}
                                        Stop recording
                                {:else}
                                        Start voice prompt
                                {/if}
                        </button>
                </div>

                {#if !$recordingSupported}
                        <p class="warning">
                                This environment does not support microphone recording via the MediaRecorder API.
                        </p>
                {/if}

                {#if $voiceSessionError}
                        <p class="error">{$voiceSessionError}</p>
                {/if}
                {#if $voiceError}
                        <p class="error">{$voiceError}</p>
                {/if}

                <div class="voice-room-preview" aria-live="polite">
                        <h3>Remote audio</h3>
                        {#if $remoteAudioTracks.length === 0}
                                <p class="muted">No remote audio tracks are active yet.</p>
                        {:else}
                                {#each $remoteAudioTracks as remote (remote.id)}
                                        <div class="remote-audio-item">
                                                <strong>{remote.participant}</strong>
                                                <audio
                                                        use:attachAudio={remote.track}
                                                        controls
                                                        autoplay
                                                        playsinline
                                                ></audio>
                                        </div>
                                {/each}
                        {/if}
                </div>

                {#if $recordedAudioUrl}
                        <div class="voice-preview">
                                <h3>Last recorded prompt</h3>
                                <audio src={$recordedAudioUrl} controls></audio>
                        </div>
                {/if}

                {#if $voiceTranscript}
                        <article class="voice-result">
                                <header>
                                        <h3>Transcript</h3>
                                </header>
                                <p>{$voiceTranscript}</p>
                        </article>
                {/if}

                {#if $voiceReplyText}
                        <article class="voice-result">
                                <header>
                                        <h3>Claude replied</h3>
                                </header>
                                <p>{$voiceReplyText}</p>
                                {#if $voiceReplyAudioUrl}
                                        <audio src={$voiceReplyAudioUrl} controls autoplay></audio>
                                {/if}
                        </article>
                {/if}
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

        .voice-session-meta {
                display: flex;
                gap: 2rem;
                flex-wrap: wrap;
                margin-bottom: 1rem;
        }

        .voice-session-meta div {
                display: flex;
                flex-direction: column;
                gap: 0.25rem;
        }

        .status-label {
                font-size: 0.75rem;
                text-transform: uppercase;
                letter-spacing: 0.08em;
                color: rgba(148, 163, 184, 0.8);
        }

        .warning {
                color: #facc15;
                font-weight: 600;
        }

        .muted {
                color: rgba(148, 163, 184, 0.75);
        }

        .voice-room-preview {
                display: flex;
                flex-direction: column;
                gap: 0.75rem;
                margin-top: 1.5rem;
        }

        .voice-room-preview h3 {
                margin: 0;
        }

        .remote-audio-item {
                display: flex;
                flex-direction: column;
                gap: 0.5rem;
                padding: 1rem;
                border-radius: 12px;
                background: rgba(15, 23, 42, 0.55);
                border: 1px solid rgba(79, 70, 229, 0.2);
        }

        .remote-audio-item audio,
        .voice-preview audio,
        .voice-result audio {
                width: 100%;
        }

        .voice-preview,
        .voice-result {
                margin-top: 1.25rem;
                background: rgba(15, 23, 42, 0.55);
                border: 1px solid rgba(79, 70, 229, 0.2);
                border-radius: 12px;
                padding: 1rem;
                display: flex;
                flex-direction: column;
                gap: 0.75rem;
        }

        .voice-result p {
                margin: 0;
                line-height: 1.5;
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
