<script lang="ts" context="module">
        export const ssr = false
</script>

<script lang="ts">
        // TypeScript declarations for Web Speech API
        interface SpeechRecognition extends EventTarget {
                continuous: boolean
                interimResults: boolean
                lang: string
                start(): void
                stop(): void
                onstart: ((this: SpeechRecognition, ev: Event) => any) | null
                onend: ((this: SpeechRecognition, ev: Event) => any) | null
                onerror: ((this: SpeechRecognition, ev: any) => any) | null
                onresult: ((this: SpeechRecognition, ev: any) => any) | null
        }

        import { derived, get, writable } from 'svelte/store'
        import { onDestroy, onMount } from 'svelte'
        import { Room, RoomEvent, Track } from 'livekit-client'
        import { taurpc } from '$lib/tauri'
        import type { ClaudeMetricsSnapshot, ClaudeVoiceResponse } from '../types'
        import { pipeline, type PipelineType, env } from '@xenova/transformers'

        // Configure transformers to use cached models
        env.allowLocalModels = false
        env.useBrowserCache = true

        let whisperPipeline: any = null
        let isLoadingWhisper = false
        const whisperReady = writable(false)

        const metrics = writable<ClaudeMetricsSnapshot | null>(null)
        const metricsLoading = writable(false)
        const metricsError = writable('')
        const nextSyncTime = writable<number | null>(null)

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

        // Audio recording variables
        let speechSynthesis: SpeechSynthesis | null = null
        let currentTranscript = ''
        let mediaRecorder: MediaRecorder | null = null
        let audioChunks: Blob[] = []
        let recordingStream: MediaStream | null = null

        let dataDir = ''
        let hoursBack = 24
        let pythonPath = ''
        let serverUrl = 'http://localhost:8080'

        let apiKey = ''
        let codeContext = ''
        let voiceModel = 'claude-sonnet-4-5'
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

        // Agent configuration variables
        let agentLivekitUrl = ''
        let agentLivekitApiKey = ''
        let agentLivekitApiSecret = ''
        let agentAnthropicApiKey = ''
        let agentCodebasePath = ''
        let agentRunning = false
        let agentPid: number | null = null
        let agentStatusMessage = ''
        let agentConfigSaved = false

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
                        return
                }

                try {
                        const result = await taurpc[''].push_claude_metrics({
                                metrics: current,
                                server_url: serverUrl,
                                auth_token: null,
                        })
                        metrics.set(result)
                        metricsError.set('')
                } catch (error) {
                        metricsError.set(error instanceof Error ? error.message : String(error))
                }
        }

        let syncInterval: number | null = null
        let countdownInterval: number | null = null
        let agentStatusInterval: number | null = null

        async function startAutoSync() {
                // Clear any existing intervals
                if (syncInterval !== null) {
                        clearInterval(syncInterval)
                }
                if (countdownInterval !== null) {
                        clearInterval(countdownInterval)
                }

                // Set next sync time to 1 minute from now
                nextSyncTime.set(Date.now() + 60000)

                // Update countdown every second
                countdownInterval = setInterval(() => {
                        const next = get(nextSyncTime)
                        if (next !== null && next <= Date.now()) {
                                nextSyncTime.set(Date.now() + 60000)
                        }
                }, 1000) as unknown as number

                // Sync metrics every minute
                syncInterval = setInterval(async () => {
                        await syncMetrics()
                        nextSyncTime.set(Date.now() + 60000)
                }, 60000) as unknown as number

                // Initial sync
                await syncMetrics()
        }

        function stopAutoSync() {
                if (syncInterval !== null) {
                        clearInterval(syncInterval)
                        syncInterval = null
                }
                if (countdownInterval !== null) {
                        clearInterval(countdownInterval)
                        countdownInterval = null
                }
                nextSyncTime.set(null)
        }

        function formatTimeUntilSync(timestamp: number | null): string {
                if (timestamp === null) return ''
                const diff = Math.max(0, timestamp - Date.now())
                const seconds = Math.floor(diff / 1000)
                const minutes = Math.floor(seconds / 60)
                const remainingSeconds = seconds % 60
                if (minutes > 0) {
                        return `${minutes}m ${remainingSeconds}s`
                }
                return `${remainingSeconds}s`
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
                
                // Use agent configuration credentials
                const url = agentLivekitUrl.trim() || livekitUrl.trim()
                const apiKey = agentLivekitApiKey.trim() || livekitApiKey.trim()
                const apiSecret = agentLivekitApiSecret.trim() || livekitApiSecret.trim()
                
                if (!url) {
                        voiceError.set('LiveKit URL not configured. Please configure the agent first.')
                        return
                }
                if (!apiKey || !apiSecret) {
                        voiceError.set('LiveKit credentials not configured. Please configure the agent first.')
                        return
                }
                if (!livekitIdentity.trim() || !livekitRoom.trim()) {
                        voiceError.set('Participant identity and room name are required.')
                        return
                }
                livekitState.set('connecting')
                try {
                        const token = await taurpc[''].generate_livekit_token({
                                api_key: apiKey,
                                api_secret: apiSecret,
                                identity: livekitIdentity,
                                room: livekitRoom,
                                name: livekitName || null,
                                metadata: null,
                                ttl_seconds: 3600,
                                can_publish: true,
                                can_subscribe: true,
                                can_publish_data: false,
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

                        await instance.connect(url, token.token)
                        await instance.localParticipant.setMicrophoneEnabled(true)
                        room = instance
                        livekitState.set('connected')
                        voiceStatusMessage = 'Connected! The agent will greet you and respond to your voice automatically.'
                } catch (error) {
                        cleanupRoom()
                        livekitState.set('disconnected')
                        voiceError.set(error instanceof Error ? error.message : String(error))
                }
        }

        async function disconnectVoiceSession() {
                voiceStatusMessage = ''
                if (mediaRecorder && mediaRecorder.state !== 'inactive') {
                        mediaRecorder.stop()
                        mediaRecorder = null
                }
                if (recordingStream) {
                        recordingStream.getTracks().forEach(track => track.stop())
                        recordingStream = null
                }
                if (window.speechSynthesis) {
                        window.speechSynthesis.cancel()
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
                console.log('startRecording called')
                voiceError.set('')
                voiceStatusMessage = ''
                voiceResponse.set(null)
                voiceAudioUrl.set('')
                currentTranscript = ''
                audioChunks = []
                
                if (!apiKey.trim()) {
                        voiceError.set('Claude API key is required before capturing audio.')
                        return
                }
                if (get(livekitState) !== 'connected') {
                        voiceError.set('Connect to LiveKit before recording a question.')
                        return
                }

                // Check if Whisper is ready
                if (!get(whisperReady)) {
                        if (isLoadingWhisper) {
                                voiceStatusMessage = 'Loading speech recognition model, please wait...'
                                // Wait for model to load
                                const checkReady = setInterval(() => {
                                        if (get(whisperReady)) {
                                                clearInterval(checkReady)
                                                startRecording()
                                        }
                                }, 500)
                                setTimeout(() => clearInterval(checkReady), 30000) // Timeout after 30s
                                return
                        } else {
                                voiceError.set('Speech recognition model not loaded. Please refresh the page.')
                                return
                        }
                }

                // Check for MediaRecorder support
                if (!window.MediaRecorder) {
                        voiceError.set('Audio recording is not supported in this browser.')
                        return
                }

                try {
                        // Get audio stream from LiveKit or microphone
                        const stream = await getAudioStream()
                        if (!stream) {
                                voiceError.set('Unable to access microphone.')
                                return
                        }

                        recordingStream = stream
                        
                        // Create MediaRecorder with WAV for better compatibility with Whisper
                        let mimeType = 'audio/webm;codecs=opus'
                        if (!MediaRecorder.isTypeSupported(mimeType)) {
                                mimeType = 'audio/webm'
                        }
                        
                        mediaRecorder = new MediaRecorder(stream, { mimeType })
                        
                        mediaRecorder.ondataavailable = (event) => {
                                if (event.data.size > 0) {
                                        audioChunks.push(event.data)
                                }
                        }
                        
                        mediaRecorder.onstop = async () => {
                                voiceRecording.set(false)
                                if (recordingStream) {
                                        recordingStream.getTracks().forEach(track => track.stop())
                                        recordingStream = null
                                }
                                
                                if (audioChunks.length === 0) {
                                        voiceError.set('No audio was recorded.')
                                        return
                                }
                                
                                // Create audio blob
                                const audioBlob = new Blob(audioChunks, { type: mimeType })
                                voiceStatusMessage = 'Transcribing with local Whisper model...'
                                
                                // Transcribe using local Whisper
                                await transcribeAudio(audioBlob)
                        }
                        
                        mediaRecorder.onerror = (event) => {
                                console.error('MediaRecorder error:', event)
                                voiceError.set('Recording error occurred.')
                                stopRecording()
                        }
                        
                        mediaRecorder.start()
                        voiceRecording.set(true)
                        voiceStatusMessage = 'Recording… click "Stop listening" when done.'
                        console.log('Recording started')
                } catch (error) {
                        console.error('Failed to start recording:', error)
                        voiceError.set(error instanceof Error ? error.message : 'Failed to start recording')
                        if (recordingStream) {
                                recordingStream.getTracks().forEach(track => track.stop())
                                recordingStream = null
                        }
                }
        }

        async function getAudioStream(): Promise<MediaStream | null> {
                // Try to get audio from LiveKit first
                if (room) {
                        const publications = Array.from(room.localParticipant.audioTrackPublications.values())
                        const publication = publications.find((pub) =>
                                pub.source === Track.Source.Microphone && pub.track,
                        )
                        if (publication?.track) {
                                const cloned = publication.track.mediaStreamTrack.clone()
                                return new MediaStream([cloned])
                        }
                }
                
                // Fall back to getUserMedia
                if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
                        voiceError.set('Media devices API is not available.')
                        return null
                }
                
                try {
                        return await navigator.mediaDevices.getUserMedia({ audio: true })
                } catch (error) {
                        console.error('getUserMedia error:', error)
                        voiceError.set(error instanceof Error ? error.message : 'Failed to access microphone')
                        return null
                }
        }

        async function transcribeAudio(audioBlob: Blob) {
                try {
                        if (!whisperPipeline) {
                                voiceError.set('Speech recognition model not loaded.')
                                return
                        }
                        
                        // Convert blob to ArrayBuffer
                        const arrayBuffer = await audioBlob.arrayBuffer()
                        
                        // Create audio context to decode and process audio
                        const audioContext = new AudioContext({ sampleRate: 16000 })
                        const audioBuffer = await audioContext.decodeAudioData(arrayBuffer)
                        
                        // Get audio data as Float32Array (Whisper expects 16kHz mono)
                        let audioData: Float32Array
                        if (audioBuffer.numberOfChannels === 1) {
                                audioData = audioBuffer.getChannelData(0)
                        } else {
                                // Convert stereo to mono by averaging channels
                                const channel1 = audioBuffer.getChannelData(0)
                                const channel2 = audioBuffer.getChannelData(1)
                                audioData = new Float32Array(channel1.length)
                                for (let i = 0; i < channel1.length; i++) {
                                        audioData[i] = (channel1[i] + channel2[i]) / 2
                                }
                        }
                        
                        console.log('Running Whisper transcription...')
                        
                        // Run Whisper transcription
                        const result = await whisperPipeline(audioData, {
                                chunk_length_s: 30,
                                stride_length_s: 5,
                                return_timestamps: false,
                        })
                        
                        const transcript = result.text?.trim() || ''
                        
                        if (!transcript) {
                                voiceError.set('No speech detected in audio.')
                                return
                        }
                        
                        console.log('Transcription result:', transcript)
                        currentTranscript = transcript
                        voiceStatusMessage = `Transcribed: "${transcript}"`
                        
                        // Send to Claude
                        await submitTextQuestion(transcript)
                        
                } catch (error) {
                        console.error('Transcription error:', error)
                        voiceError.set(error instanceof Error ? error.message : 'Transcription failed')
                }
        }

        async function stopRecording() {
                if (mediaRecorder && mediaRecorder.state !== 'inactive') {
                        try {
                                mediaRecorder.stop()
                        } catch (error) {
                                console.error('Error stopping recorder:', error)
                        }
                }
        }

        async function submitTextQuestion(transcript: string) {
                voiceLoading.set(true)
                voiceError.set('')
                try {
                        const result = await taurpc[''].ask_claude({
                                api_key: apiKey,
                                question: transcript,
                                code_context: codeContext || null,
                                system_prompt: systemPrompt || null,
                                model: voiceModel || null,
                                max_output_tokens: voiceMaxTokens || null,
                                temperature: voiceTemperature || null,
                        })

                        voiceResponse.set({
                                answer_text: result.answer,
                                answer_audio_base64: null,
                                answer_audio_mime_type: null,
                                transcript: transcript,
                                model: result.model,
                                stop_reason: result.stop_reason || null,
                                usage: result.usage || null,
                        })
                        
                        voiceStatusMessage = 'Response received. Speaking answer…'
                        
                        // Use text-to-speech to speak the response
                        if (autoPlayVoice && result.answer) {
                                speakText(result.answer)
                        }
                } catch (error) {
                        voiceError.set(error instanceof Error ? error.message : String(error))
                } finally {
                        voiceLoading.set(false)
                }
        }

        function speakText(text: string) {
                if (!('speechSynthesis' in window)) {
                        voiceError.set('Text-to-speech is not supported in this browser.')
                        return
                }

                // Cancel any ongoing speech
                window.speechSynthesis.cancel()

                const utterance = new SpeechSynthesisUtterance(text)
                utterance.lang = 'en-US'
                utterance.rate = 1.0
                utterance.pitch = 1.0
                utterance.volume = 1.0

                utterance.onstart = () => {
                        voiceStatusMessage = 'Speaking response…'
                }

                utterance.onend = () => {
                        voiceStatusMessage = 'Finished speaking.'
                }

                utterance.onerror = (event) => {
                        console.error('Speech synthesis error:', event)
                        voiceError.set('Text-to-speech error: ' + event.error)
                }

                window.speechSynthesis.speak(utterance)
        }

        // Agent management functions
        async function loadAgentConfig() {
                try {
                        const config = await taurpc[''].load_agent_config()
                        if (config) {
                                agentLivekitUrl = config.livekit_url
                                agentLivekitApiKey = config.livekit_api_key
                                agentLivekitApiSecret = config.livekit_api_secret
                                agentAnthropicApiKey = config.anthropic_api_key
                                agentCodebasePath = config.codebase_path || ''
                                agentConfigSaved = true
                                agentStatusMessage = 'Configuration loaded successfully'
                        } else {
                                agentStatusMessage = 'No saved configuration found'
                        }
                } catch (error) {
                        agentStatusMessage = 'Error loading configuration: ' + (error instanceof Error ? error.message : String(error))
                }
        }

        async function saveAgentConfig() {
                try {
                        if (!agentLivekitUrl.trim() || !agentLivekitApiKey.trim() || !agentLivekitApiSecret.trim() || !agentAnthropicApiKey.trim()) {
                                agentStatusMessage = 'All fields are required'
                                return
                        }
                        
                        await taurpc[''].save_agent_config({
                                livekit_url: agentLivekitUrl,
                                livekit_api_key: agentLivekitApiKey,
                                livekit_api_secret: agentLivekitApiSecret,
                                anthropic_api_key: agentAnthropicApiKey,
                                codebase_path: agentCodebasePath.trim() || null,
                        })
                        agentConfigSaved = true
                        agentStatusMessage = 'Configuration saved successfully!'
                } catch (error) {
                        agentStatusMessage = 'Error saving configuration: ' + (error instanceof Error ? error.message : String(error))
                }
        }

        async function startAgent() {
                try {
                        if (!agentConfigSaved) {
                                agentStatusMessage = 'Please save configuration first'
                                return
                        }
                        
                        await taurpc[''].start_agent()
                        agentStatusMessage = 'Agent starting...'
                        
                        // Wait a moment then check status
                        setTimeout(checkAgentStatus, 2000)
                } catch (error) {
                        agentStatusMessage = 'Error starting agent: ' + (error instanceof Error ? error.message : String(error))
                }
        }

        async function stopAgent() {
                try {
                        await taurpc[''].stop_agent()
                        agentRunning = false
                        agentPid = null
                        agentStatusMessage = 'Agent stopping...'
                        // Wait a bit for the process to fully stop, then check status
                        setTimeout(async () => {
                                await checkAgentStatus()
                        }, 1000)
                } catch (error) {
                        agentStatusMessage = 'Error stopping agent: ' + (error instanceof Error ? error.message : String(error))
                }
        }

        async function checkAgentStatus() {
                try {
                        const status = await taurpc[''].get_agent_status()
                        agentRunning = status.running
                        agentPid = status.pid || null
                        
                        if (status.running) {
                                agentStatusMessage = `Agent running (PID: ${status.pid})`
                        } else {
                                agentStatusMessage = 'Agent not running'
                        }
                } catch (error) {
                        agentStatusMessage = 'Error checking status: ' + (error instanceof Error ? error.message : String(error))
                }
        }

        onMount(async () => {
                // Load agent configuration on startup
                await loadAgentConfig()
                await checkAgentStatus()
                
                // Check agent status every 5 seconds
                agentStatusInterval = setInterval(checkAgentStatus, 5000) as unknown as number
                
                // Initialize Whisper model in the background
                if (!isLoadingWhisper && !whisperPipeline) {
                        isLoadingWhisper = true
                        try {
                                console.log('Loading Whisper model (this may take a minute on first run)...')
                                // Use the tiny model for faster loading (6x smaller than base)
                                // The model will be cached in the browser's cache storage
                                whisperPipeline = await pipeline(
                                        'automatic-speech-recognition', 
                                        'Xenova/whisper-tiny.en',
                                        { 
                                                quantized: true,
                                                progress_callback: (progress: any) => {
                                                        if (progress.status === 'downloading') {
                                                                console.log(`Downloading ${progress.file}: ${Math.round(progress.progress)}%`)
                                                        }
                                                }
                                        }
                                )
                                whisperReady.set(true)
                                console.log('Whisper model loaded successfully')
                        } catch (error) {
                                console.error('Failed to load Whisper model:', error)
                                voiceError.set('Failed to load speech recognition model. Please refresh the page.')
                        } finally {
                                isLoadingWhisper = false
                        }
                }
        })

        onDestroy(() => {
                stopAutoSync()
                if (agentStatusInterval !== null) {
                        clearInterval(agentStatusInterval)
                        agentStatusInterval = null
                }
                if (mediaRecorder && mediaRecorder.state !== 'inactive') {
                        mediaRecorder.stop()
                        mediaRecorder = null
                }
                if (recordingStream) {
                        recordingStream.getTracks().forEach(track => track.stop())
                        recordingStream = null
                }
                if (window.speechSynthesis) {
                        window.speechSynthesis.cancel()
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
                </div>

                <div class="actions">
                        <button class="primary" onclick={loadMetrics} disabled={$metricsLoading}>
                                {#if $metricsLoading}
                                        Collecting…
                                {:else}
                                        Collect metrics
                                {/if}
                        </button>
                        {#if $nextSyncTime === null}
                                <button class="secondary" onclick={startAutoSync} disabled={!$metrics}>
                                        Start auto-sync
                                </button>
                        {:else}
                                <button class="secondary" onclick={stopAutoSync}>
                                        Stop auto-sync
                                </button>
                                <div class="sync-timer">
                                        Next update in: {formatTimeUntilSync($nextSyncTime)}
                                </div>
                        {/if}
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
                <header>
                        <h1>🤖 Voice Agent Configuration</h1>
                        <p>
                                Configure and manage the LiveKit voice agent. The agent uses LiveKit Inference for
                                speech-to-text (Deepgram) and text-to-speech (Cartesia), plus Claude for AI responses.
                        </p>
                        <p>
                                <strong>Codebase Awareness:</strong> The agent can read files, search code, and explore any project you point it to.
                                Set a codebase path below to analyze a specific project, or leave empty to use the agent's working directory.
                        </p>
                </header>

                <div class="voice-grid">
                        <label>
                                LiveKit URL
                                <input
                                        placeholder="wss://your-project.livekit.cloud"
                                        bind:value={agentLivekitUrl}
                                />
                        </label>
                        <label>
                                LiveKit API Key
                                <input
                                        placeholder="APIxxxxxxxxxx"
                                        bind:value={agentLivekitApiKey}
                                />
                        </label>
                        <label>
                                LiveKit API Secret
                                <input
                                        type="password"
                                        placeholder="••••••••••••••••"
                                        bind:value={agentLivekitApiSecret}
                                />
                        </label>
                        <label>
                                Anthropic API Key
                                <input
                                        type="password"
                                        placeholder="sk-ant-••••••••••••••••"
                                        bind:value={agentAnthropicApiKey}
                                />
                        </label>
                        <label>
                                Codebase Path (optional)
                                <input
                                        placeholder="/path/to/your/codebase"
                                        bind:value={agentCodebasePath}
                                />
                                <small style="opacity: 0.7;">Leave empty to use agent's working directory</small>
                        </label>
                </div>

                <div style="display: flex; gap: 0.5rem; margin-top: 1rem; flex-wrap: wrap;">
                        <button onclick={saveAgentConfig} style="flex: 1; min-width: 150px;">
                                💾 Save Configuration
                        </button>
                        <button
                                onclick={startAgent}
                                disabled={!agentConfigSaved || agentRunning}
                                style="flex: 1; min-width: 150px;"
                        >
                                ▶️ Start Agent
                        </button>
                        <button
                                onclick={stopAgent}
                                disabled={!agentRunning}
                                style="flex: 1; min-width: 150px;"
                        >
                                ⏹️ Stop Agent
                        </button>
                        <button
                                onclick={checkAgentStatus}
                                style="flex: 1; min-width: 150px;"
                        >
                                🔄 Check Status
                        </button>
                </div>

                {#if agentStatusMessage}
                        <div style="margin-top: 1rem; padding: 0.75rem; border: 1px solid {agentRunning ? '#c3e6cb' : '#dee2e6'}; border-radius: 4px; font-size: 0.9rem;">
                                <strong>Status:</strong> {agentStatusMessage}
                        </div>
                {/if}

                <details style="margin-top: 1rem; padding: 0.5rem; border-radius: 4px;">
                        <summary style="cursor: pointer; font-weight: bold;">ℹ️ How to get API keys</summary>
                        <div style="margin-top: 0.5rem; font-size: 0.9rem; line-height: 1.6;">
                                <p><strong>LiveKit Cloud:</strong></p>
                                <ol style="margin-left: 1.5rem;">
                                        <li>Go to <a href="https://cloud.livekit.io" target="_blank">cloud.livekit.io</a></li>
                                        <li>Sign up/login (free tier available)</li>
                                        <li>Create a project and copy the URL, API Key, and API Secret</li>
                                </ol>
                                <p style="margin-top: 0.5rem;"><strong>Anthropic API:</strong></p>
                                <ol style="margin-left: 1.5rem;">
                                        <li>Go to <a href="https://console.anthropic.com" target="_blank">console.anthropic.com</a></li>
                                        <li>Sign up/login and add credits</li>
                                        <li>Create an API key</li>
                                </ol>
                                <p style="margin-top: 0.5rem;"><em>💡 The agent will automatically use LiveKit Inference for STT/TTS</em></p>
                        </div>
                </details>
        </section>

                <section class="panel">
                <div class="sr-only" aria-hidden="true" bind:this={remoteAudioContainer}></div>
                <header>
                        <h1>🎙️ Connect to Voice Session</h1>
                        <p>
                                Once the agent is running above, connect to start talking. The agent automatically handles everything - just speak naturally!
                        </p>
                </header>

                <div class="voice-grid">
                        <label>
                                Participant identity
                                <input
                                        placeholder="my-laptop"
                                        bind:value={livekitIdentity}
                                />
                        </label>
                        <label>
                                Room name
                                <input placeholder="my-voice-room" bind:value={livekitRoom} />
                        </label>
                        <label>
                                Display name (optional)
                                <input placeholder="User" bind:value={livekitName} />
                        </label>
                </div>

                <div class="voice-actions">
                        {#if $livekitState === 'connected'}
                                <button class="secondary" onclick={disconnectVoiceSession}>
                                        Disconnect
                                </button>
                        {:else}
                                <button
                                        class="primary"
                                        onclick={connectVoiceSession}
                                        disabled={$livekitState === 'connecting'}
                                >
                                        {#if $livekitState === 'connecting'}
                                                Connecting…
                                        {:else}
                                                Connect to Voice Session
                                        {/if}
                                </button>
                        {/if}
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
                        <button class="secondary" onclick={generateLivekitToken}>
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

        .sync-timer {
                display: flex;
                align-items: center;
                padding: 0.65rem 1.3rem;
                background: rgba(56, 189, 248, 0.15);
                border: 1px solid rgba(56, 189, 248, 0.35);
                border-radius: 999px;
                color: #94e2ff;
                font-size: 0.95rem;
                font-weight: 600;
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
