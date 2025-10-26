<script lang="ts">
	import { Check, Dot } from "@lucide/svelte";
	import { quintIn, quintOut } from "svelte/easing";
	import { fly } from "svelte/transition";
	import { Button } from "@/components/ui/button";
	import { Input } from "@/components/ui/input";
	import { Label } from "@/components/ui/label";
	import Spinner from "@/components/ui/spinner/spinner.svelte";
	import { Alert, AlertDescription } from "@/components/ui/alert";
	import { Badge } from "@/components/ui/badge";
	import {
		Card,
		CardContent,
		CardDescription,
		CardFooter,
		CardHeader,
		CardTitle,
	} from "@/components/ui/card";
	import { cn } from "@/utils";
	import { taurpc } from "@/tauri";
	import { isConnected } from "@/shared.svelte";

	const Step = {
		BLE: "ble",
		WN: "wifi_name",
		WP: "wifi_pass",
		CONN: "connected",
	} as const;
	type StepType = (typeof Step)[keyof typeof Step];
	type Direction = -1 | 0 | 1;

	const stepOrder: StepType[] = [Step.BLE, Step.WN, Step.WP, Step.CONN];
	const connectionChecklist = [
		"Bluetooth link established",
		"Wi‑Fi credentials synced",
		"GitHub token stored",
	] as const;

	let gh_token = $state("");
	let wifi_name = $state("");
	let wifi_pass = $state("");
	let stepIndex = $state(0);
	let isConnecting = $state(false);
	let errorMessage = $state("");
	let direction = $state<Direction>(0);
	let lastConnection = $state<{ wifiName: string } | null>(null);

	const stepMeta: Record<StepType, { title: string; description: string }> = {
		[Step.BLE]: {
			title: "Connect GitHub",
			description:
				"Paste the personal access token you generated for Cappy so we can sync repositories.",
		},
		[Step.WN]: {
			title: "Choose your network",
			description:
				"Enter the Wi‑Fi name exactly as it appears on your devices.",
		},
		[Step.WP]: {
			title: "Secure the connection",
			description:
				"Add the Wi‑Fi password so we can send credentials to the device.",
		},
		[Step.CONN]: {
			title: "Cappy is ready",
			description: "Your device is paired, connected, and ready to code.",
		},
	};

	const interactiveSteps = stepOrder.slice(0, -1);
	const totalInputSteps = interactiveSteps.length;

	const step = $derived(stepOrder[stepIndex]);
	const currentMeta = $derived(stepMeta[step]);
	const currentStepNumber = $derived.by(() => {
		if (step === Step.CONN) return totalInputSteps;
		const index = interactiveSteps.indexOf(step);
		return index >= 0 ? index + 1 : totalInputSteps;
	});
	const canContinue = $derived.by(() => {
		switch (step) {
			case Step.BLE:
				return gh_token.trim().length > 0;
			case Step.WN:
				return wifi_name.trim().length > 0;
			case Step.WP:
				return wifi_pass.trim().length > 0;
			default:
				return true;
		}
	});
	const enterOffset = $derived(
		direction === 1 ? 64 : direction === -1 ? -64 : 0,
	);
	const exitOffset = $derived(
		direction === 1 ? -64 : direction === -1 ? 64 : 0,
	);

	function go(delta: -1 | 1) {
		const next = stepIndex + delta;
		if (next < 0 || next >= stepOrder.length) return;
		direction = delta;
		stepIndex = next;
		errorMessage = "";
		if (stepOrder[next] !== Step.CONN) {
			isConnected.set(false);
		}
	}

	async function connectDevice(event: Event) {
		event.preventDefault();
		const token = gh_token.trim();
		const gh_user = "kenricqq";
		const ssid = wifi_name.trim();
		const password = wifi_pass.trim();

		if (!token || !ssid || !password) {
			errorMessage = "Please complete every field before connecting.";
			return;
		}

		try {
			isConnecting = true;
			errorMessage = "";
			const response = await taurpc.connect_device(
				token,
				gh_user,
				ssid,
				password,
			);
			console.log(response);
			lastConnection = { wifiName: ssid };
			gh_token = "";
			wifi_name = "";
			wifi_pass = "";
			direction = 1;
			stepIndex = stepOrder.indexOf(Step.CONN);
			isConnected.set(true);
		} catch (error) {
			errorMessage = "Connection failed. Try again in a moment.";
		} finally {
			isConnecting = false;
		}
	}

	function jumpTo(index: number) {
		if (index < 0 || index >= stepOrder.length || index === stepIndex) {
			return;
		}
		direction = index > stepIndex ? 1 : -1;
		stepIndex = index;
		if (stepOrder[index] !== Step.CONN) {
			isConnected.set(false);
		}
		errorMessage = "";
	}

	function restart() {
		gh_token = "";
		wifi_name = "";
		wifi_pass = "";
		direction = -1;
		stepIndex = 0;
		isConnected.set(false);
		errorMessage = "";
		lastConnection = null;
	}
</script>

<main class="container mx-auto min-h-[calc(100vh-4rem)] py-12">
	<div class="grid gap-8 lg:grid-cols-[minmax(0,0.85fr)_minmax(0,1fr)]">
		<Card
			class="relative h-full overflow-hidden border border-border/70 bg-slate-950 text-slate-100 shadow-2xl"
		>
			<div
				aria-hidden="true"
				class="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(56,189,248,0.18),transparent_65%)]"
			></div>

			<CardHeader class="space-y-3">
				<Badge
					variant="outline"
					class="w-fit border-slate-700/80 bg-white/5 text-slate-100"
				>
					Setup guide
				</Badge>
				<CardTitle class="text-3xl font-semibold tracking-tight"
					>Pair your Cappy</CardTitle
				>
				<CardDescription class="text-slate-300">
					Follow the guided steps to give your board access to GitHub and your
					Wi‑Fi network.
				</CardDescription>
			</CardHeader>

			<CardContent class="flex flex-col gap-5">
				<ol class="flex flex-col gap-4">
					{#each stepOrder as stepName, index (stepName)}
						{@const isActive = step === stepName}
						{@const isComplete = stepIndex > index}
						<li>
							<button
								type="button"
								class={cn(
									"flex w-full items-start gap-4 rounded-2xl border p-4 text-left transition disabled:cursor-not-allowed disabled:opacity-50",
									isActive
										? "border-cyan-400/80 bg-cyan-500/10"
										: isComplete
											? "border-emerald-400/60 bg-emerald-500/10"
											: "border-slate-700/70 bg-slate-900/70 hover:border-cyan-400/60 hover:bg-slate-900/80",
								)}
								onclick={() => jumpTo(index)}
								disabled={index > stepIndex}
							>
								<span
									class={cn(
										"flex h-9 w-9 items-center justify-center rounded-full border text-sm font-semibold",
										isActive
											? "border-cyan-300 bg-cyan-300/10 text-cyan-100"
											: isComplete
												? "border-emerald-300 bg-emerald-400/20 text-emerald-50"
												: "border-slate-600 text-slate-200",
									)}
								>
									{#if isComplete}
										<Check class="h-4 w-4" />
									{:else}
										{index + 1}
									{/if}
								</span>

								<div class="space-y-1">
									<p
										class="text-sm font-semibold uppercase tracking-wide text-slate-200/90"
									>
										{stepMeta[stepName].title}
									</p>
									<p class="text-sm text-slate-300">
										{stepMeta[stepName].description}
									</p>
								</div>
							</button>
						</li>
					{/each}
				</ol>

				<p
					class="rounded-2xl border border-slate-700/70 bg-slate-900/70 p-4 text-sm text-slate-300"
				>
					<strong class="text-slate-100">Pro tip:</strong> You can revisit a completed
					step to tweak the details anytime.
				</p>
			</CardContent>
		</Card>

		<Card class="h-full border border-border/70 bg-card/95 shadow-xl">
			<form class="flex h-full flex-col gap-6" onsubmit={connectDevice}>
				<CardHeader class="space-y-3 pb-0">
					<div class="flex items-center justify-between gap-3">
						<Badge variant="secondary">
							{#if step === Step.CONN}
								All set
							{:else}
								Step {currentStepNumber} of {totalInputSteps}
							{/if}
						</Badge>
					</div>
					<CardTitle class="text-2xl font-semibold tracking-tight">
						{currentMeta.title}
					</CardTitle>
					<CardDescription class="text-muted-foreground">
						{currentMeta.description}
					</CardDescription>
				</CardHeader>

				<CardContent class="flex flex-1 flex-col gap-6 pb-0">
					{#if step === Step.CONN}
						<div
							class="flex flex-col gap-6"
							in:fly={{
								x: enterOffset,
								duration: 240,
								easing: quintOut,
							}}
							out:fly={{
								x: exitOffset,
								duration: 200,
								easing: quintIn,
							}}
						>
							<div
								class="inline-flex items-center gap-2 rounded-full bg-emerald-500/10 px-3 py-1 text-sm font-medium text-emerald-600 dark:text-emerald-300"
							>
								<Dot class="h-3 w-3" />
								Device connected
							</div>

							<div class="space-y-2">
								<h3 class="text-xl font-semibold text-foreground">
									You're good to go!
								</h3>
								<p class="text-sm text-muted-foreground">
									We saved your credentials locally on the board. You can unplug
									and jump straight into Cappy Coding.
								</p>
							</div>

							<ul class="space-y-3">
								{#each connectionChecklist as item (item)}
									<li
										class="flex items-center gap-3 rounded-xl border border-emerald-400/40 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-700 dark:text-emerald-100"
									>
										<Check class="h-4 w-4" />
										<span>{item}</span>
									</li>
								{/each}
							</ul>

							{#if lastConnection}
								<div
									class="rounded-xl border border-border/70 bg-muted/40 px-4 py-3 text-sm text-muted-foreground"
								>
									Last network:
									<span class="font-medium text-foreground"
										>{lastConnection.wifiName}</span
									>
								</div>
							{/if}
						</div>
					{:else if step === Step.BLE}
						<div
							class="space-y-4"
							in:fly={{
								x: enterOffset,
								duration: 240,
								easing: quintOut,
							}}
							out:fly={{
								x: exitOffset,
								duration: 200,
								easing: quintIn,
							}}
						>
							<div class="space-y-2">
								<Label
									for="gh_token"
									class="text-sm font-medium text-foreground"
								>
									GitHub personal access token
								</Label>
								<Input
									id="gh_token"
									placeholder="gh_asdf12345"
									bind:value={gh_token}
									autocomplete="off"
								/>
								<p class="text-sm text-muted-foreground">
									Create a fine-grained token with the repository scopes you
									need.
								</p>
							</div>
						</div>
					{:else if step === Step.WN}
						<div
							class="space-y-4"
							in:fly={{
								x: enterOffset,
								duration: 240,
								easing: quintOut,
							}}
							out:fly={{
								x: exitOffset,
								duration: 200,
								easing: quintIn,
							}}
						>
							<div class="space-y-2">
								<Label
									for="wifi_name"
									class="text-sm font-medium text-foreground"
								>
									Wi‑Fi network name (SSID)
								</Label>
								<Input
									id="wifi_name"
									placeholder="NETGEAR-5G"
									bind:value={wifi_name}
									autocomplete="off"
									spellcheck={false}
								/>
								<p class="text-sm text-muted-foreground">
									This value is case-sensitive.
								</p>
							</div>
						</div>
					{:else if step === Step.WP}
						<div
							class="space-y-4"
							in:fly={{
								x: enterOffset,
								duration: 240,
								easing: quintOut,
							}}
							out:fly={{
								x: exitOffset,
								duration: 200,
								easing: quintIn,
							}}
						>
							<div class="space-y-2">
								<Label
									for="wifi_pass"
									class="text-sm font-medium text-foreground"
								>
									Wi‑Fi password
								</Label>
								<Input
									id="wifi_pass"
									type="password"
									placeholder="cappyCoding!"
									bind:value={wifi_pass}
									autocomplete="current-password"
								/>
								<p class="text-sm text-muted-foreground">
									We only store this locally on your device.
								</p>
							</div>
						</div>
					{/if}

					{#if errorMessage}
						<Alert variant="destructive">
							<AlertDescription>{errorMessage}</AlertDescription>
						</Alert>
					{/if}
				</CardContent>

				<CardFooter class="flex items-center justify-between gap-3 pt-0">
					{#if stepIndex > 0 && step !== Step.CONN}
						<Button
							type="button"
							variant="ghost"
							onclick={() => go(-1)}
							disabled={isConnecting}
						>
							Back
						</Button>
					{:else}
						<div></div>
					{/if}

					{#if step === Step.CONN}
						<Button type="button" onclick={restart}>Run setup again</Button>
					{:else if step === Step.WP}
						<Button
							type="submit"
							class="min-w-[9rem]"
							disabled={!canContinue || isConnecting}
						>
							{#if isConnecting}
								<Spinner class="mr-2" />
								<span>Connecting...</span>
							{:else}
								Connect
							{/if}
						</Button>
					{:else}
						<Button
							type="button"
							class="min-w-[8rem]"
							onclick={() => go(1)}
							disabled={!canContinue}
						>
							Continue
						</Button>
					{/if}
				</CardFooter>
			</form>
		</Card>
	</div>
</main>
