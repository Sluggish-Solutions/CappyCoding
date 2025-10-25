<script lang="ts">
	import { Button } from '@/components/ui/button'
	import '../app.css'

	let { children } = $props()
</script>

<div class="h-screen">
	<div class="">
		<Button href="/">Home</Button>
		<Button href="/call_api">call_api</Button>
	</div>
	{@render children()}
</div>
