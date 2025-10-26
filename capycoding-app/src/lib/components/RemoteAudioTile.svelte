<script lang="ts">
        import { onDestroy } from 'svelte'
        import { setupMediaTrack } from '@livekit/components-core'
        import { Track, type RemoteParticipant, type RemoteTrackPublication } from 'livekit-client'
        import type { Subscription } from 'rxjs'

        export let participant: RemoteParticipant
        export let publication: RemoteTrackPublication

        let audioElement: HTMLAudioElement | null = null
        let trackSubscription: Subscription | null = null
        let detachTrack: (() => void) | null = null
        let mediaClass = ''

        function subscribeToPublication(currentPublication: RemoteTrackPublication | null) {
                trackSubscription?.unsubscribe()
                trackSubscription = null
                detachTrack?.()
                detachTrack = null

                if (!audioElement || !participant || !currentPublication) {
                        return
                }

                const element = audioElement
                if (!element) {
                        return
                }

                const { className, trackObserver } = setupMediaTrack({
                        participant,
                        publication: currentPublication,
                        source: currentPublication.source,
                })
                mediaClass = className
                trackSubscription = trackObserver.subscribe((nextPublication) => {
                        const mediaTrack = nextPublication?.track
                        if (mediaTrack && nextPublication?.isSubscribed && mediaTrack.kind === Track.Kind.Audio) {
                                mediaTrack.attach(element)
                                element.autoplay = true
                                element.controls = true
                                detachTrack = () => mediaTrack.detach(element)
                        } else if (mediaTrack) {
                                mediaTrack.detach(element)
                                detachTrack = null
                        }
                })
        }

        $: subscribeToPublication(publication ?? null)

        onDestroy(() => {
                trackSubscription?.unsubscribe()
                trackSubscription = null
                detachTrack?.()
                detachTrack = null
        })
</script>

<div class="lk-remote-audio">
        <header>
                <h4>{participant.name ?? participant.identity}</h4>
        </header>
        <audio bind:this={audioElement} class={mediaClass}></audio>
</div>

<style>
        .lk-remote-audio {
                display: grid;
                gap: 0.35rem;
                background: rgba(15, 23, 42, 0.55);
                border-radius: 0.75rem;
                padding: 0.75rem 1rem;
                border: 1px solid rgba(148, 163, 184, 0.15);
        }

        h4 {
                margin: 0;
                font-size: 0.9rem;
                font-weight: 600;
        }

        audio {
                width: 100%;
        }
</style>
