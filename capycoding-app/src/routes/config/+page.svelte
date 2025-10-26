<script lang="ts">
	import { type DragDropState, draggable, droppable } from '@thisux/sveltednd'
	import { flip } from 'svelte/animate'
	import { fade } from 'svelte/transition'
	import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card'
	import { Badge } from '$lib/components/ui/badge'

	type ID = string

	type Widget = {
		id: ID
		abbr: string
		desc: string
		content: string
	}

	type PlacedWidget = {
		instanceId: string
		widgetId: ID
	}

	type PageState = {
		id: string
		title: string
		widgets: PlacedWidget[]
	}

	type WorkspaceState = {
		widgets: Record<ID, Widget>
		pages: PageState[]
	}

	type LibraryPayload = { source: 'library'; widgetId: ID }
	type BoardPayload = {
		source: 'board'
		pageId: string
		index: number
		instanceId: string
		widgetId: ID
	}

	type DragPayload = LibraryPayload | BoardPayload

	type DropTarget =
		| { kind: 'library' }
		| { kind: 'page'; pageId: string }
		| { kind: 'slot'; pageId: string; slotIndex: number }

	const MAX_WIDGETS_PER_PAGE = 2

	const INITIAL_WORKSPACE: WorkspaceState = {
		widgets: {
			'github-summary': {
				id: 'github-summary',
				abbr: 'GitHub PRs',
				desc: 'Pull requests, commits and issues at a glance',
				content: 'PRs: 30 · Commits: 55'
			},
			'claude-usage': {
				id: 'claude-usage',
				abbr: 'Claude Code',
				desc:
					'Token usage across the workspace with live updates',
				content: 'Used: 30k · Remaining: 93k'
			},
			'github-growth': {
				id: 'github-growth',
				abbr: 'GitHub Stars',
				desc: 'Stars gained and followers trend',
				content: 'Followers: 30 · Stars: 20'
			},
			'github-actions': {
				id: 'github-actions',
				abbr: 'GitHub Workflows',
				desc: 'Workflow activity and automation health',
				content: 'Runs: 12 · Failures: 1'
			},
			'custom-text': {
				id: 'custom-text',
				abbr: 'Custom',
				desc: 'Custom notes or metrics you want to highlight',
				content: 'Add something meaningful here'
			}
		},
		pages: [
			{
				id: 'page-0',
				title: 'Overview',
				widgets: [
					{ instanceId: 'slot-0', widgetId: 'github-summary' }
				]
			},
			{
				id: 'page-1',
				title: 'Analytics',
				widgets: [
					{ instanceId: 'slot-2', widgetId: 'github-growth' }
				]
			},
			{
				id: 'page-2',
				title: 'Team',
				widgets: []
			}
		]
	}

	let workspace = $state<WorkspaceState>(
		structuredClone(INITIAL_WORKSPACE)
	)

	const pages = $derived(workspace.pages)
	const libraryWidgets = $derived(Object.values(workspace.widgets))

	let instanceSeed = 0
	function nextInstanceId() {
		instanceSeed += 1
		return `instance-${instanceSeed}`
	}

	export function loadWorkspace(data: WorkspaceState) {
		workspace = structuredClone(data)
	}

	export function exportWorkspace(): WorkspaceState {
		return structuredClone(workspace)
	}

	function findPage(pageId: string) {
		return workspace.pages.find((candidate) =>
			candidate.id === pageId
		) ?? null
	}

	function clampSlotIndex(index: number) {
		return Math.max(0, Math.min(index, MAX_WIDGETS_PER_PAGE - 1))
	}

	function parseTarget(container: string): DropTarget | null {
		if (container === 'library') {
			return { kind: 'library' }
		}

		if (container.startsWith('page:')) {
			return { kind: 'page', pageId: container.slice(5) }
		}

		const slotMatch = /^slot:(.+):(\d+)$/.exec(container)
		if (slotMatch) {
			return {
				kind: 'slot',
				pageId: slotMatch[1],
				slotIndex: Number.parseInt(slotMatch[2], 10)
			}
		}

		return null
	}

	function moveWidget(
		state: DragDropState<DragPayload> & { targetIndex?: number }
	) {
		const { draggedItem, targetContainer, targetIndex } = state
		if (!targetContainer) return

		const target = parseTarget(targetContainer)
		if (!target) return

		if (target.kind === 'library') {
			if (draggedItem.source === 'board') {
				const originPage = findPage(draggedItem.pageId)
				if (!originPage) return
				originPage.widgets.splice(draggedItem.index, 1)
			}
			return
		}

		const targetPage = findPage(target.pageId)
		if (!targetPage) return

		let desiredIndex: number
		let replaceExisting: boolean

		if (target.kind === 'slot') {
			desiredIndex = clampSlotIndex(target.slotIndex)
			replaceExisting = desiredIndex < targetPage.widgets.length
		} else {
			const fallbackIndex = targetIndex
				?? targetPage.widgets.length
			desiredIndex = clampSlotIndex(fallbackIndex)
			replaceExisting = desiredIndex < targetPage.widgets.length
		}

		if (draggedItem.source === 'library') {
			handleLibraryDrop(
				targetPage,
				desiredIndex,
				replaceExisting,
				draggedItem
			)
			return
		}

		handleBoardDrop(
			targetPage,
			desiredIndex,
			replaceExisting,
			draggedItem
		)
	}

	function handleLibraryDrop(
		page: PageState,
		slotIndex: number,
		replaceExisting: boolean,
		payload: LibraryPayload
	) {
		const newEntry: PlacedWidget = {
			instanceId: nextInstanceId(),
			widgetId: payload.widgetId
		}
		const capacityReached =
			page.widgets.length >= MAX_WIDGETS_PER_PAGE

		if (replaceExisting) {
			if (slotIndex < page.widgets.length) {
				page.widgets.splice(slotIndex, 1, newEntry)
				return
			}
		}

		if (capacityReached) return

		const insertIndex = Math.min(slotIndex, page.widgets.length)
		page.widgets.splice(insertIndex, 0, newEntry)
	}

	function handleBoardDrop(
		targetPage: PageState,
		slotIndex: number,
		replaceExisting: boolean,
		payload: BoardPayload
	) {
		const originPage = findPage(payload.pageId)
		if (!originPage) return

		const movingCard = originPage.widgets[payload.index]
		if (
			!movingCard || movingCard.instanceId !== payload.instanceId
		) {
			return
		}

		const samePage = originPage.id === targetPage.id

		originPage.widgets.splice(payload.index, 1)
		const restoreIndex = Math.min(
			payload.index,
			originPage.widgets.length
		)
		const restoreToOrigin = () => {
			originPage.widgets.splice(restoreIndex, 0, movingCard)
		}

		if (samePage) {
			let insertIndex = slotIndex
			if (!replaceExisting && insertIndex > payload.index) {
				insertIndex -= 1
			}

			if (
				replaceExisting
				&& insertIndex < originPage.widgets.length
			) {
				originPage.widgets.splice(insertIndex, 1, movingCard)
				return
			}

			if (originPage.widgets.length >= MAX_WIDGETS_PER_PAGE) {
				restoreToOrigin()
				return
			}

			insertIndex = Math.min(
				insertIndex,
				originPage.widgets.length
			)
			originPage.widgets.splice(insertIndex, 0, movingCard)
			return
		}

		if (replaceExisting && slotIndex < targetPage.widgets.length) {
			targetPage.widgets.splice(slotIndex, 1, movingCard)
			return
		}

		if (targetPage.widgets.length >= MAX_WIDGETS_PER_PAGE) {
			restoreToOrigin()
			return
		}

		const insertIndex = Math.min(
			slotIndex,
			targetPage.widgets.length
		)
		targetPage.widgets.splice(insertIndex, 0, movingCard)
	}
</script>

<div class="flex gap-6 overflow-x-auto p-4">
	<div
		class="w-72 flex-none"
		use:droppable={{
			container: 'library',
			callbacks: { onDrop: moveWidget },
			attributes: {
				draggingClass:
					'ring-2 ring-primary/25 bg-primary/5 rounded-lg'
			}
		}}
	>
		<Card class="h-full">
			<CardHeader class="space-y-1">
				<CardTitle class="text-lg font-semibold">
					Widget Library
				</CardTitle>
				<CardDescription class="text-xs">
					Drag into a page slot to add it
				</CardDescription>
			</CardHeader>

			<CardContent class="space-y-3">
				{#each libraryWidgets as widget (widget.id)}
					<article
						use:draggable={{
							container: 'library',
							dragData: {
								source: 'library',
								widgetId: widget.id
							}
						}}
						in:fade={{ duration: 120 }}
						out:fade={{ duration: 120 }}
						class="cursor-grab rounded-lg border border-border bg-muted/50 p-3 transition-colors hover:bg-background hover:shadow-sm"
					>
						<div class="flex items-center justify-between">
							<strong
								class="text-sm font-semibold text-foreground"
							>
								{widget.abbr}
							</strong>
							<Badge
								variant="outline"
								class="uppercase tracking-wide text-[0.65rem]"
							>
								LIB
							</Badge>
						</div>
						<p class="mt-2 text-xs text-muted-foreground">
							{widget.desc}
						</p>
					</article>
				{/each}

				{#if libraryWidgets.length === 0}
					<p
						class="rounded-lg border border-dashed border-border/60 p-3 text-center text-xs text-muted-foreground"
					>
						Library empty — drag widgets back from pages to store
						them here
					</p>
				{/if}
			</CardContent>
		</Card>
	</div>

	<div class="flex w-full min-w-[32rem] gap-6">
		{#each pages as page (page.id)}
			<Card class="flex-1 bg-muted/30 shadow-inner">
				<CardHeader class="flex items-center justify-between gap-2">
					<CardTitle class="text-base font-semibold">
						{page.title}
					</CardTitle>
					<Badge
						variant="outline"
						class="uppercase tracking-wider text-[0.65rem]"
					>
						Page {page.id.split('-')[1]}
					</Badge>
				</CardHeader>

				<CardContent class="flex flex-col gap-4">
					{#each Array(2) as _, index (index)}
						{@const placed = page.widgets[index]}
						<div
							class="group relative flex min-h-[8rem] items-stretch justify-stretch rounded-xl border border-dashed border-transparent bg-background/80 p-2 transition-all duration-200 ease-linear hover:border-muted-foreground/40 hover:bg-muted/40"
							class:opacity-60={!placed}
							use:droppable={{
								container: `slot:${page.id}:${index}`,
								callbacks: { onDrop: moveWidget },
								attributes: {
									draggingClass:
										'border-primary/60 bg-primary/10 ring-2 ring-primary/30 shadow-sm'
								}
							}}
							animate:flip={{ duration: 200 }}
						>
							{#if placed}
								{#key placed.instanceId}
									{@const widget = workspace
							.widgets[placed.widgetId]}
									{#if widget}
										<article
											use:draggable={{
												container: page.id,
												dragData: {
													source: 'board',
													pageId: page.id,
													index,
													instanceId:
														placed.instanceId,
													widgetId: widget.id
												}
											}}
											class="flex h-32 w-full cursor-grab flex-col justify-between rounded-lg bg-gradient-to-r from-sky-500 to-blue-600 p-4 text-white shadow-lg transition-[transform,box-shadow] duration-200 hover:shadow-blue-500/40 active:scale-95"
										>
											<div
												class="flex items-center justify-between text-xs uppercase tracking-wider text-white/80"
											>
												<span>{widget.abbr}</span>
												<span>Slot {
														index
														+ 1
													}</span>
											</div>
											<p
												class="text-sm font-medium leading-snug"
											>
												{widget.content}
											</p>
										</article>
									{:else}
										<div
											class="flex h-full w-full items-center justify-center rounded-lg bg-muted/40 px-4 text-[0.7rem] font-semibold uppercase tracking-[0.08em] text-muted-foreground"
										>
											Missing widget definition
										</div>
									{/if}
								{/key}
							{:else}
								<div
									class="flex h-full w-full items-center justify-center rounded-lg bg-muted/40 px-4 text-[0.7rem] font-semibold uppercase tracking-[0.08em] text-muted-foreground"
								>
									Drop widget here
								</div>
							{/if}
						</div>
					{/each}
				</CardContent>
			</Card>
		{/each}
	</div>
</div>
