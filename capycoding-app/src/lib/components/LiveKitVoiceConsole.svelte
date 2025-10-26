<script lang="ts">
        import { onDestroy, onMount } from 'svelte'
        import { get, writable } from 'svelte/store'
        import '@livekit/components-styles'
        import {
                createMediaDeviceObserver,
                setupDeviceSelector,
                setupLiveKitRoom,
                setupStartAudio,
        } from '@livekit/components-core'
        import type { Subscription } from 'rxjs'
        import {
                Room,
                RoomEvent,
                Track,
                type LocalAudioTrack,
                type RemoteParticipant,
                type RemoteTrackPublication,
        } from 'livekit-client'
        import { taurpc } from '$lib/tauri'
        import RemoteAudioTile from './RemoteAudioTile.svelte'
        import type { ClaudeVoicePromptResponse, LivekitTokenResponse } from '../../types'

        export let serverUrl = ''
        export let apiKey = ''
        export let apiSecret = ''
        export let identity = ''
        export let roomName = ''
        export let displayName = ''
        export let metadata = ''
        export let ttlSeconds = 3600
        export let canPublish = true
        export let canSubscribe = true
        export let canPublishData = true

        export let agentUrl = ''
        export let agentApiKey = ''
        export let agentId = ''
        export let agentVoice = ''

        type RemoteAudioEntry = {
                id: string
                participant: RemoteParticipant
                publication: RemoteTrackPublication
        }

        const { className: roomContainerClass } = setupLiveKitRoom()
        const {
                className: startAudioClass,
                roomAudioPlaybackAllowedObservable,
                handleStartAudioPlayback,
        } = setupStartAudio()

        const connectionState = writable<'idle' | 'connecting' | 'connected'>('idle')
        const sessionError = writable('')
        const promptError = writable('')
        const promptLoading = writable(false)
        const recordingSupported = writable(false)
        const isRecording = writable(false)
        const transcript = writable('')
        const replyText = writable('')
        const replyAudioUrl = writable('')
        const recordedAudioUrl = writable('')
        const sessionId = writable('')
        const microphones = writable<MediaDeviceInfo[]>([])
        const selectedMicrophone = writable('')
        const remoteAudioPublications = writable<RemoteAudioEntry[]>([])
        const deviceSelectClass = writable('lk-media-device-select')
        const playbackRequiresInteraction = writable(false)

        let room: Room | null = null
        let recorder: MediaRecorder | null = null
        let recorderStream: MediaStream | null = null
        let recorderChunks: Blob[] = []
        let mediaObserver: { unsubscribe: () => void } | null = null
        let playbackSubscription: Subscription | null = null
        let micSelector: ReturnType<typeof setupDeviceSelector> | null = null
        let micSelectorSubscription: Subscription | null = null

        // @ts-expect-error TauRPC namespaces are keyed by an empty string
        const rpc = taurpc['']

        function resetPromptArtifacts() {
                const previewUrl = get(recordedAudioUrl)
                if (previewUrl) {
                        URL.revokeObjectURL(previewUrl)
                        recordedAudioUrl.set('')
                }
                const replyUrl = get(replyAudioUrl)
                if (replyUrl) {
                        URL.revokeObjectURL(replyUrl)
                        replyAudioUrl.set('')
                }
        }

        function clearRemoteAudioPublications() {
                remoteAudioPublications.set([])
        }

        function cleanupRoom() {
                clearRemoteAudioPublications()
                micSelectorSubscription?.unsubscribe()
                micSelectorSubscription = null
                micSelector = null
                playbackSubscription?.unsubscribe()
                playbackSubscription = null
                if (room) {
                        room.removeAllListeners()
                        room = null
                }
        }

        async function generateToken(): Promise<LivekitTokenResponse | undefined> {
                sessionError.set('')
                try {
                        const result = await rpc.generate_livekit_token({
                                api_key: apiKey,
                                api_secret: apiSecret,
                                identity,
                                room: roomName,
                                name: displayName ? displayName : null,
                                metadata: metadata ? metadata : null,
                                ttl_seconds: Number.isFinite(ttlSeconds) ? ttlSeconds : null,
                                can_publish: canPublish,
                                can_subscribe: canSubscribe,
                                can_publish_data: canPublishData,
                        })
                        return result
                } catch (error) {
                        sessionError.set(error instanceof Error ? error.message : String(error))
                        return undefined
                }
        }

        function observePlayback(activeRoom: Room) {
                playbackSubscription?.unsubscribe()
                playbackSubscription = roomAudioPlaybackAllowedObservable(activeRoom).subscribe(
                        ({ canPlayAudio }) => {
                                playbackRequiresInteraction.set(!canPlayAudio)
                        },
                )
        }

        function configureDeviceSelector(activeRoom: Room) {
                micSelectorSubscription?.unsubscribe()
                const track = activeRoom.localParticipant
                        .getTrackPublication(Track.Source.Microphone)
                        ?.track as LocalAudioTrack | undefined
                micSelector = setupDeviceSelector('audioinput', activeRoom, track)
                deviceSelectClass.set(micSelector.className)
                micSelectorSubscription = micSelector.activeDeviceObservable.subscribe((deviceId) => {
                        if (deviceId) {
                                selectedMicrophone.set(deviceId)
                        }
                })
        }

        function registerRoomListeners(activeRoom: Room) {
                activeRoom.on(RoomEvent.ConnectionStateChanged, (state) => {
                        if (state === 'connected') {
                                connectionState.set('connected')
                                sessionError.set('')
                        } else if (state === 'disconnected') {
                                connectionState.set('idle')
                        } else if (state === 'connecting') {
                                connectionState.set('connecting')
                        }
                })

                activeRoom.on(RoomEvent.Disconnected, () => {
                        connectionState.set('idle')
                        playbackRequiresInteraction.set(false)
                        cleanupRoom()
                })

                activeRoom.on(RoomEvent.TrackSubscribed, (track, publication, participant) => {
                        if (track.kind === Track.Kind.Audio) {
                                const remoteParticipant = participant as RemoteParticipant
                                const remotePublication = publication as RemoteTrackPublication
                                const publicationSid =
                                        remotePublication.trackSid ||
                                        `${remoteParticipant.identity}-${Math.random().toString(36).slice(2)}`
                                remoteAudioPublications.update((entries) => {
                                        if (entries.some((entry) => entry.id === publicationSid)) {
                                                return entries
                                        }
                                        return [
                                                ...entries,
                                                {
                                                        id: publicationSid,
                                                        participant: remoteParticipant,
                                                        publication: remotePublication,
                                                },
                                        ]
                                })
                        }
                })

                activeRoom.on(RoomEvent.TrackUnsubscribed, (track, publication, participant) => {
                        if (track?.kind === Track.Kind.Audio) {
                                const publicationSid = publication.trackSid
                                remoteAudioPublications.update((entries) =>
                                        entries.filter((entry) => entry.id !== publicationSid),
                                )
                        }
                })

                activeRoom.on(RoomEvent.ParticipantDisconnected, (participant) => {
                        remoteAudioPublications.update((entries) =>
                                entries.filter((entry) => entry.participant.identity !== participant.identity),
                        )
                })
        }

        async function connectRoom() {
                if (get(connectionState) === 'connecting') {
                        return
                }
                if (!serverUrl.trim()) {
                        sessionError.set('Provide the LiveKit server URL before connecting.')
                        return
                }
                if (!apiKey.trim() || !apiSecret.trim() || !identity.trim() || !roomName.trim()) {
                        sessionError.set(
                                'LiveKit API key, secret, identity, and room are required to join the session.',
                        )
                        return
                }

                try {
                        const permissionStream = await navigator.mediaDevices.getUserMedia({ audio: true })
                        permissionStream.getTracks().forEach((mediaTrack) => mediaTrack.stop())
                } catch (error) {
                        sessionError.set(
                                error instanceof Error
                                        ? `Microphone permission denied: ${error.message}`
                                        : `Microphone permission denied: ${String(error)}`,
                        )
                        return
                }

                connectionState.set('connecting')
                const tokenResult = await generateToken()
                if (!tokenResult) {
                        connectionState.set('idle')
                        return
                }

                await disconnectRoom(true)

                const audioDevice = get(selectedMicrophone)
                const nextRoom = new Room({
                        webAudioMix: true,
                        audioCaptureDefaults: audioDevice
                                ? {
                                          deviceId: audioDevice,
                                          echoCancellation: true,
                                          noiseSuppression: true,
                                  }
                                : undefined,
                })
                registerRoomListeners(nextRoom)
                room = nextRoom

                try {
                        await nextRoom.connect(serverUrl, tokenResult.token)
                        observePlayback(nextRoom)
                        try {
                                await nextRoom.startAudio()
                        } catch (error) {
                                console.warn('Automatic audio start failed', error)
                        }
                        const micDevice = get(selectedMicrophone)
                        try {
                                await nextRoom.localParticipant.setMicrophoneEnabled(true, {
                                        deviceId: micDevice || undefined,
                                })
                        } catch (error) {
                                console.warn('failed to enable microphone', error)
                        }
                        configureDeviceSelector(nextRoom)
                        connectionState.set('connected')
                        sessionError.set('')
                } catch (error) {
                        await disconnectRoom(false)
                        sessionError.set(error instanceof Error ? error.message : String(error))
                        connectionState.set('idle')
                }
        }

        async function disconnectRoom(stopTracks: boolean) {
                if (!room) return
                await stopRecording()
                try {
                        await room.localParticipant.setMicrophoneEnabled(false)
                } catch (error) {
                        console.warn('failed to disable microphone', error)
                }
                try {
                        await room.disconnect(stopTracks)
                } catch (error) {
                        console.warn('failed to disconnect LiveKit room', error)
                }
                playbackRequiresInteraction.set(false)
                cleanupRoom()
                connectionState.set('idle')
        }

        async function handleStartAudio() {
                if (!room) return
                try {
                        await handleStartAudioPlayback(room)
                } catch (error) {
                        sessionError.set(error instanceof Error ? error.message : String(error))
                }
        }

        async function onMicrophoneChange(event: Event) {
                const target = event.currentTarget as HTMLSelectElement
                const deviceId = target.value
                selectedMicrophone.set(deviceId)
                if (micSelector) {
                        try {
                                await micSelector.setActiveMediaDevice(deviceId, { exact: true })
                        } catch (error) {
                                sessionError.set(
                                        error instanceof Error
                                                ? `Failed to switch microphone: ${error.message}`
                                                : `Failed to switch microphone: ${String(error)}`,
                                )
                        }
                }
        }

        async function startRecording() {
                promptError.set('')
                if (get(isRecording)) {
                        return
                }
                if (get(promptLoading)) {
                        promptError.set('Wait for the previous prompt to finish processing.')
                        return
                }
                if (get(connectionState) !== 'connected') {
                        promptError.set('Join the LiveKit room before recording a prompt.')
                        return
                }
                try {
                        const deviceId = get(selectedMicrophone)
                        const constraints: MediaStreamConstraints = {
                                audio: deviceId ? { deviceId: { exact: deviceId } } : true,
                        }
                        recorderStream = await navigator.mediaDevices.getUserMedia(constraints)
                        recorderChunks = []
                        recorder = new MediaRecorder(recorderStream)
                        recorder.ondataavailable = (event) => {
                                if (event.data.size > 0) {
                                        recorderChunks.push(event.data)
                                }
                        }
                        recorder.onstop = handleRecordingComplete
                        recorder.start()
                        isRecording.set(true)
                } catch (error) {
                        promptError.set(error instanceof Error ? error.message : String(error))
                        if (recorderStream) {
                                recorderStream.getTracks().forEach((track) => track.stop())
                                recorderStream = null
                        }
                }
        }

        function handleRecordingComplete() {
                if (!recorderChunks.length) {
                        promptError.set('No audio captured from the microphone.')
                        return
                }
                const blob = new Blob(recorderChunks, { type: recorderChunks[0]?.type || 'audio/webm' })
                recorderChunks = []
                const previewUrl = get(recordedAudioUrl)
                if (previewUrl) {
                        URL.revokeObjectURL(previewUrl)
                }
                recordedAudioUrl.set(URL.createObjectURL(blob))
                void submitVoicePrompt(blob)
        }

        async function stopRecording() {
                if (!get(isRecording)) {
                        return
                }
                recorder?.stop()
                recorder = null
                if (recorderStream) {
                        recorderStream.getTracks().forEach((track) => track.stop())
                        recorderStream = null
                }
                isRecording.set(false)
        }

        async function submitVoicePrompt(blob: Blob) {
                promptLoading.set(true)
                promptError.set('')
                try {
                        const base64 = await blobToBase64(blob)
                        const mimeType = blob.type || 'audio/webm'
                        const activeRoom = room
                        const lkRoomName = activeRoom?.name ?? null
                        const lkIdentity = activeRoom?.localParticipant?.identity ?? null
                        const response: ClaudeVoicePromptResponse = await rpc.relay_livekit_audio({
                                api_key: agentApiKey,
                                agent_url: agentUrl,
                                audio_base64: base64,
                                mime_type: mimeType,
                                agent_id: agentId ? agentId : null,
                                response_voice: agentVoice ? agentVoice : null,
                                session_id: get(sessionId) || null,
                                room_name: lkRoomName,
                                participant_identity: lkIdentity,
                        })
                        transcript.set(response.transcript || '')
                        replyText.set(response.reply_text || '')
                        sessionId.set(response.session_id ?? '')
                        const existingReply = get(replyAudioUrl)
                        if (existingReply) {
                                URL.revokeObjectURL(existingReply)
                        }
                        if (response.reply_audio_base64 && response.reply_audio_mime_type) {
                                const replyBlob = base64ToBlob(
                                        response.reply_audio_base64,
                                        response.reply_audio_mime_type,
                                )
                                const replyUrl = URL.createObjectURL(replyBlob)
                                replyAudioUrl.set(replyUrl)
                                if (activeRoom) {
                                        await publishReplyToRoom(activeRoom, replyBlob)
                                }
                        } else {
                                replyAudioUrl.set('')
                        }
                } catch (error) {
                        promptError.set(error instanceof Error ? error.message : String(error))
                } finally {
                        promptLoading.set(false)
                }
        }

        async function publishReplyToRoom(activeRoom: Room, blob: Blob) {
                const arrayBuffer = await blob.arrayBuffer()
                const context = new AudioContext()
                const buffer = await context.decodeAudioData(arrayBuffer.slice(0))
                const source = context.createBufferSource()
                source.buffer = buffer
                const destination = context.createMediaStreamDestination()
                source.connect(destination)
                source.connect(context.destination)
                source.start(0)
                const track = destination.stream.getAudioTracks()[0]
                if (!track) {
                        source.stop()
                        return
                }
                try {
                        await activeRoom.localParticipant.publishTrack(track, {
                                name: 'claude-reply',
                                source: Track.Source.Unknown,
                        })
                        source.onended = () => {
                                void activeRoom.localParticipant.unpublishTrack(track, true)
                                track.stop()
                                source.disconnect()
                                destination.stream.getTracks().forEach((mediaTrack) => mediaTrack.stop())
                                void context.close()
                        }
                } catch (error) {
                        console.warn('failed to publish reply audio', error)
                        track.stop()
                        source.disconnect()
                        destination.stream.getTracks().forEach((mediaTrack) => mediaTrack.stop())
                        void context.close()
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

        onMount(() => {
                let supported = false
                try {
                        supported =
                                typeof navigator !== 'undefined' &&
                                typeof navigator.mediaDevices !== 'undefined' &&
                                typeof navigator.mediaDevices.getUserMedia === 'function' &&
                                typeof window !== 'undefined' &&
                                typeof (window as typeof window & { MediaRecorder?: typeof MediaRecorder }).MediaRecorder !==
                                        'undefined'
                } catch (error) {
                        console.warn('MediaRecorder detection failed', error)
                }
                recordingSupported.set(supported)

                mediaObserver = createMediaDeviceObserver('audioinput', (error) => {
                        console.error('Failed to enumerate audio input devices', error)
                }, true).subscribe((devices) => {
                        microphones.set(devices)
                        if (!get(selectedMicrophone) && devices.length > 0) {
                                selectedMicrophone.set(devices[0].deviceId)
                        }
                })

                return () => {
                        mediaObserver?.unsubscribe()
                        mediaObserver = null
                        playbackSubscription?.unsubscribe()
                        playbackSubscription = null
                        micSelectorSubscription?.unsubscribe()
                        micSelectorSubscription = null
                        void disconnectRoom(true)
                        resetPromptArtifacts()
                }
        })

        onDestroy(() => {
                mediaObserver?.unsubscribe()
                mediaObserver = null
                playbackSubscription?.unsubscribe()
                playbackSubscription = null
                micSelectorSubscription?.unsubscribe()
                micSelectorSubscription = null
                void disconnectRoom(true)
                resetPromptArtifacts()
        })
</script>

<div class={`voice-console ${roomContainerClass}`}>
        <div class="console-header">
                <div>
                        <span class="status-label">Connection</span>
                        <strong>{$connectionState}</strong>
                </div>
                {#if $sessionId}
                        <div>
                                <span class="status-label">Session</span>
                                <code>{$sessionId}</code>
                        </div>
                {/if}
                <div>
                        <span class="status-label">Microphone</span>
                        {#if $microphones.length === 0}
                                <span class="muted">No input devices</span>
                        {:else}
                                <select
                                        class={$deviceSelectClass}
                                        bind:value={$selectedMicrophone}
                                        on:change={onMicrophoneChange}
                                        disabled={$connectionState === 'connecting'}
                                >
                                        {#each $microphones as device}
                                                <option value={device.deviceId}>{device.label || 'Microphone'}</option>
                                        {/each}
                                </select>
                        {/if}
                </div>
        </div>

        <div class="console-actions">
                <button
                        on:click={$connectionState === 'connected' ? () => disconnectRoom(true) : connectRoom}
                        disabled={$connectionState === 'connecting'}
                >
                        {#if $connectionState === 'connected'}
                                Disconnect
                        {:else if $connectionState === 'connecting'}
                                Connecting…
                        {:else}
                                Join LiveKit room
                        {/if}
                </button>
                <button
                        class={startAudioClass}
                        on:click={handleStartAudio}
                        disabled={!$playbackRequiresInteraction || $connectionState !== 'connected'}
                >
                        Enable room audio
                </button>
                <button
                        on:click={$isRecording ? stopRecording : startRecording}
                        disabled={$connectionState !== 'connected' || !$recordingSupported || $promptLoading}
                >
                        {#if $promptLoading}
                                Processing…
                        {:else if $isRecording}
                                Stop & submit
                        {:else}
                                Record voice prompt
                        {/if}
                </button>
        </div>

        {#if !$recordingSupported}
                <p class="warning">
                        Voice dictation requires a browser with MediaRecorder support. Please switch to a compatible browser.
                </p>
        {/if}

        {#if $sessionError}
                <p class="error">{$sessionError}</p>
        {/if}

        {#if $promptError}
                <p class="error">{$promptError}</p>
        {/if}

        {#if $remoteAudioPublications.length > 0}
                <div class="voice-room-preview">
                        <h3>Remote replies</h3>
                        {#each $remoteAudioPublications as remote (remote.id)}
                                <RemoteAudioTile participant={remote.participant} publication={remote.publication} />
                        {/each}
                </div>
        {/if}

        {#if $recordedAudioUrl}
                <div class="voice-preview">
                        <h3>Last recorded prompt</h3>
                        <audio src={$recordedAudioUrl} controls></audio>
                </div>
        {/if}

        {#if $transcript}
                <article class="voice-result">
                        <header>
                                <h3>Transcript</h3>
                        </header>
                        <p>{$transcript}</p>
                </article>
        {/if}

        {#if $replyText}
                <article class="voice-result">
                        <header>
                                <h3>Claude replied</h3>
                        </header>
                        <p>{$replyText}</p>
                        {#if $replyAudioUrl}
                                <audio src={$replyAudioUrl} controls autoplay></audio>
                        {/if}
                </article>
        {/if}
</div>

<style>
        .voice-console {
                display: flex;
                flex-direction: column;
                gap: 1.25rem;
        }

        .console-header {
                display: grid;
                gap: 1rem;
                grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
                align-items: end;
        }

        .status-label {
                display: block;
                font-size: 0.75rem;
                text-transform: uppercase;
                letter-spacing: 0.08em;
                color: rgba(226, 232, 240, 0.6);
        }

        select {
                width: 100%;
                padding: 0.5rem 0.75rem;
                border-radius: 0.75rem;
                border: 1px solid rgba(148, 163, 184, 0.2);
                background: rgba(15, 23, 42, 0.45);
                color: inherit;
        }

        .console-actions {
                display: flex;
                flex-wrap: wrap;
                gap: 1rem;
        }

        button {
                border: none;
                border-radius: 999px;
                background: linear-gradient(135deg, #60a5fa, #a855f7);
                color: #0f172a;
                font-weight: 600;
                padding: 0.75rem 1.75rem;
                cursor: pointer;
        }

        button[disabled] {
                opacity: 0.6;
                cursor: not-allowed;
        }

        .warning {
                padding: 0.75rem 1rem;
                border-radius: 0.75rem;
                background: rgba(251, 191, 36, 0.1);
                color: #fbbf24;
        }

        .error {
                padding: 0.75rem 1rem;
                border-radius: 0.75rem;
                background: rgba(239, 68, 68, 0.1);
                color: #f87171;
        }

        .muted {
                color: rgba(226, 232, 240, 0.55);
        }

        .voice-room-preview,
        .voice-preview,
        .voice-result {
                background: rgba(15, 23, 42, 0.45);
                border-radius: 1rem;
                padding: 1rem 1.25rem;
                border: 1px solid rgba(148, 163, 184, 0.12);
        }

        .voice-room-preview {
                display: grid;
                gap: 1rem;
        }

        audio {
                width: 100%;
        }
</style>
