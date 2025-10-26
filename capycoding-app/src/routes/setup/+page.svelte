<script lang="ts">
	import { Circle, Dot } from '@lucide/svelte'
	import { quintIn, quintOut } from 'svelte/easing'
	import { fly } from 'svelte/transition'
	import { Button } from '@/components/ui/button'
	import { Input } from '@/components/ui/input'
	import { Label } from '@/components/ui/label'
	import Spinner from '@/components/ui/spinner/spinner.svelte'
	import { taurpc } from '@/tauri'
	import { isConnected } from '@/shared.svelte'

	const Step = {
		BLE: 'ble',
		WN: 'wifi_name',
		WP: 'wifi_pass',
		CONN: 'connected'
	} as const
	type StepType = typeof Step[keyof typeof Step]
	type Direction = -1 | 0 | 1

	const stepOrder: StepType[] = [
		Step.BLE,
		Step.WN,
		Step.WP,
		Step.CONN
	]
	const connectionChecklist = [
		'Bluetooth link established',
		'Wi‑Fi credentials synced',
		'GitHub token stored'
	] as const

	let gh_token = $state('')
	let wifi_name = $state('')
	let wifi_pass = $state('')
	let stepIndex = $state(0)
	let isConnecting = $state(false)
	let errorMessage = $state('')
	let direction = $state<Direction>(0)
	let lastConnection = $state<{ wifiName: string } | null>(null)

	$inspect(errorMessage)

	const step = $derived(stepOrder[stepIndex])
	const enterOffset = $derived(
		direction === 1 ? 64 : direction === -1 ? -64 : 0
	)
	const exitOffset = $derived(
		direction === 1 ? -64 : direction === -1 ? 64 : 0
	)

	function go(delta: -1 | 1) {
		const next = stepIndex + delta
		if (next < 0 || next >= stepOrder.length) return
		direction = delta
		stepIndex = next
		errorMessage = ''
		if (stepOrder[next] !== Step.CONN) {
			isConnected.set(false)
		}
	}

	async function connectDevice(event: Event) {
		event.preventDefault()
		if (!gh_token || !wifi_name || !wifi_pass) {
			errorMessage =
				'Please fill in every field before connecting.'
			return
		}

		try {
			isConnecting = true
			errorMessage = ''
			const response = await taurpc.connect_device(
				gh_token,
				wifi_name,
				wifi_pass
			)
			console.log(response)
			lastConnection = { wifiName: wifi_name }
			gh_token = ''
			wifi_name = ''
			wifi_pass = ''
			direction = 1
			stepIndex = stepOrder.indexOf(Step.CONN)
			isConnected.set(true)
		} catch (error) {
			errorMessage = 'Connection failed. Try again in a moment.'
		} finally {
			isConnecting = false
		}
	}

	function jumpTo(index: number) {
		if (
			index < 0 || index >= stepOrder.length
			|| index === stepIndex
		) return
		direction = index > stepIndex ? 1 : -1
		stepIndex = index
		if (stepOrder[index] !== Step.CONN) {
			isConnected.set(false)
		}
		errorMessage = ''
	}

	function restart() {
		gh_token = ''
		wifi_name = ''
		wifi_pass = ''
		direction = -1
		stepIndex = 0
		isConnected.set(false)
		errorMessage = ''
		lastConnection = null
	}
</script>

<!--
	<main class="container grid">
	<div class="place-self-center min-w-32 max-w-52">
-->
<main class="container grid h-full place-items-center place-self-center mb-10">
	<div
		class="min-w-sm max-w-lg dark:bg-slate-900/50 bg-slate-100/50 p-20 rounded-lg outline"
	>
		{#key step}
			{#if step === Step.BLE}
				{@render stepper(
				'gh_token',
				'Github Token',
				'gh_asdf12345',
				() => gh_token,
				(value) => (gh_token = value),
				0
			)}
			{:else if step === Step.WN}
				{@render stepper(
				'wifi_name',
				'Wi‑Fi Name',
				'NETGEAR-5G',
				() => wifi_name,
				(value) => (wifi_name = value),
				1
			)}
			{:else if step === Step.WP}
				{@render stepper(
				'wifi_pass',
				'Wi‑Fi Password',
				'cappyCoding!',
				() => wifi_pass,
				(value) => (wifi_pass = value),
				2,
				'password',
				true
			)}
			{:else if step === Step.CONN}
				<aside
					class="card row-start-1 col-start-1 flex flex-col gap-4"
					in:fly={{
						x: enterOffset,
						duration: 240,
						easing: quintOut
					}}
					out:fly={{
						x: exitOffset,
						duration: 200,
						easing: quintIn
					}}
				>
					<h2 class="flex items-center gap-2 text-lg font-semibold">
						<Circle class="size-5 text-primary" />
						<span>Device connected</span>
					</h2>
					<p class="text-sm text-muted-foreground">
						{#if isConnected.get()}
							Board is ready and now joined&nbsp;
							<strong>
								{
									lastConnection?.wifiName
									?? 'your Wi‑Fi'
								}
							</strong>. You can head over to the connection
							commands and start coding.
						{:else}
							Connection details unavailable. Run the setup steps
							again to pair your board.
						{/if}
					</p>
					<ul class="space-y-2 text-sm text-muted-foreground">
						{#each connectionChecklist as item}
							<li class="flex items-center gap-2">
								<Dot class="size-4 text-primary/80" />
								<span>{item}</span>
							</li>
						{/each}
					</ul>
					<div class="mt-auto flex justify-end">
						<Button
							type="button"
							variant="outline"
							onclick={restart}
						>
							Set up another device
						</Button>
					</div>
				</aside>
			{:else}
				<aside>All done! You're ready to vibe code!</aside>
			{/if}
		{/key}
	</div>
</main>

{#snippet stepper(
	id: string,
	label: string,
	placeholder: string,
	getValue: () => string,
	setValue: (value: string) => void,
	index: number,
	inputType: string = 'text',
	isFinal: boolean = false
)}
	<aside
		class="card row-start-1 col-start-1 flex flex-col gap-4"
		in:fly={{
			x: enterOffset,
			duration: 240,
			easing: quintOut
		}}
		out:fly={{
			x: exitOffset,
			duration: 200,
			easing: quintIn
		}}
	>
		<Label for={id}>{label}</Label>
		<Input
			{id}
			{placeholder}
			type={inputType}
			bind:value={getValue, setValue}
		/>
		<!--
			{#if isFinal && errorMessage}
				<p class="text-sm text-destructive mt-2">
					{errorMessage}
				</p>
			{/if}
		-->

		<div
			class={`controls mt-auto flex gap-2 ${
				index > 0 ? 'justify-between' : 'justify-end'
			}`}
		>
			{#if index > 0}
				<Button
					type="button"
					variant="outline"
					onclick={() => go(-1)}
				>
					Back
				</Button>
			{/if}
			{#if !isFinal}
				<Button type="button" onclick={() => go(1)}>Next</Button>
			{:else}
				<Button
					type="button"
					onclick={connectDevice}
					disabled={isConnecting}
				>
					{#if isConnecting}
						<div class="animate-spin">
							<Spinner />
						</div>
					{:else}
						Connect
					{/if}
				</Button>
			{/if}
		</div>
	</aside>
{/snippet}
