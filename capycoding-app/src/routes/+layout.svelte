<script lang="ts">
	import '../app.css'
	import CircleIcon from '@lucide/svelte/icons/circle'
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check'
	import CircleHelpIcon from '@lucide/svelte/icons/circle-help'
	import MoonIcon from '@lucide/svelte/icons/moon'
	import SunIcon from '@lucide/svelte/icons/sun'
	import { ModeWatcher, toggleMode } from 'mode-watcher'
	import type { HTMLAttributes } from 'svelte/elements'
	import { onNavigate } from '$app/navigation'
	import { page } from '$app/state'
	import { Button } from '$lib/components/ui/button/index.js'
	import * as NavigationMenu from '$lib/components/ui/navigation-menu/index.js'
	import { navigationMenuTriggerStyle } from '$lib/components/ui/navigation-menu/navigation-menu-trigger.svelte'
	import { cn } from '$lib/utils.js'
	import { isConnected } from '@/shared.svelte'

	onNavigate((navigation) => {
		if (!document.startViewTransition) return

		return new Promise((resolve) => {
			document.startViewTransition(async () => {
				resolve()
				await navigation.complete
			})
		})
	})

	const components: {
		title: string
		href: string
		description: string
	}[] = [
		{
			title: 'Alert Dialog',
			href: '/docs/components/alert-dialog',
			description:
				'A modal dialog that interrupts the user with important content and expects a response.'
		},
		{
			title: 'Hover Card',
			href: '/docs/components/hover-card',
			description:
				'For sighted users to preview content available behind a link.'
		},
		{
			title: 'Progress',
			href: '/docs/components/progress',
			description:
				'Displays an indicator showing the completion progress of a task, typically displayed as a progress bar.'
		},
		{
			title: 'Scroll-area',
			href: '/docs/components/scroll-area',
			description: 'Visually or semantically separates content.'
		},
		{
			title: 'Tabs',
			href: '/docs/components/tabs',
			description:
				'A set of layered sections of content—known as tab panels—that are displayed one at a time.'
		},
		{
			title: 'Tooltip',
			href: '/docs/components/tooltip',
			description:
				'A popup that displays information related to an element when the element receives keyboard focus or the mouse hovers over it.'
		}
	]

	type ListItemProps = HTMLAttributes<HTMLAnchorElement> & {
		title: string
		href: string
		content: string
	}

	isConnected.set(false)

	let { children } = $props()
</script>

<div class="min-h-screen sm:min-h-dvh flex flex-col">
	<section class="flex justify-between items-center p-5" id="navbar">
		{#snippet ListItem({
			title,
			content,
			href,
			class: className,
			...restProps
		}: ListItemProps)}
			<li>
				<NavigationMenu.Link>
					{#snippet child()}
						<a
							{href}
							class={cn(
								'hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground block select-none space-y-1 rounded-md p-3 leading-none no-underline outline-none transition-colors',
								className
							)}
							{...restProps}
						>
							<div class="text-sm font-medium leading-none">
								{title}
							</div>
							<p
								class="text-muted-foreground line-clamp-2 text-sm leading-snug"
							>
								{content}
							</p>
						</a>
					{/snippet}
				</NavigationMenu.Link>
			</li>
		{/snippet}

		<NavigationMenu.Root viewport={false}>
			<NavigationMenu.List>
				<NavigationMenu.Item id="setup">
					<NavigationMenu.Link>
						{#snippet child()}
							<a href="/" class={navigationMenuTriggerStyle()}>
								<img
									src="/cappy-darkk.png"
									alt="cappy dark"
									class="absolute h-[1.2rem] w-[1.2rem] fade-in-30 scale-0 !transition-all dark:fade-out-30 dark:scale-100"
								/>
								<img
									src="/cappy-pure.png"
									alt="cappy"
									class="h-[1.2rem] w-[1.2rem] fade-out-30 scale-100 !transition-all dark:fade-in-30 dark:scale-0"
								/>
							</a>
						{/snippet}
					</NavigationMenu.Link>
				</NavigationMenu.Item>
				<NavigationMenu.Item id="setup">
					<NavigationMenu.Link>
						{#snippet child()}
							<a
								href="/test"
								class={navigationMenuTriggerStyle()}
							>Setup</a>
						{/snippet}
					</NavigationMenu.Link>
				</NavigationMenu.Item>
				<NavigationMenu.Item id="setup">
					<NavigationMenu.Link>
						{#snippet child()}
							<a
								href="/config"
								class={navigationMenuTriggerStyle()}
							>Config</a>
						{/snippet}
					</NavigationMenu.Link>
				</NavigationMenu.Item>
			</NavigationMenu.List>
		</NavigationMenu.Root>

		<!-- dark/light mode toggle -->
		<Button onclick={toggleMode} variant="outline" size="icon">
			<SunIcon
				class="h-[1.2rem] w-[1.2rem] rotate-0 scale-100 !transition-all dark:-rotate-90 dark:scale-0"
			/>
			<MoonIcon
				class="absolute h-[1.2rem] w-[1.2rem] rotate-90 scale-0 !transition-all dark:rotate-0 dark:scale-100"
			/>
			<span class="sr-only">Toggle theme</span>
		</Button>
	</section>

	<ModeWatcher />

	<main class="flex-1 grid px-10 py-5">
		{@render children()}
	</main>
</div>
