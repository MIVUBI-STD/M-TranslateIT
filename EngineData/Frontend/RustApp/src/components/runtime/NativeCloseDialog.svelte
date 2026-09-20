<script lang="ts">
  import { Dialog } from "bits-ui";
  import type { CloseDialogAction } from "../../app/runtime/closePolicy";

  let {
    open = $bindable(false),
    title,
    message,
    action,
    busy = false,
    primaryLabel,
    onKeepOpen,
    onPrimary,
  }: {
    open?: boolean;
    title: string;
    message: string;
    action: CloseDialogAction;
    busy?: boolean;
    primaryLabel: string;
    onKeepOpen: () => void;
    onPrimary: () => void | Promise<void>;
  } = $props();
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-50 bg-[var(--ti-overlay)] backdrop-blur-[2px]" />
    <Dialog.Content class="fixed left-1/2 top-1/2 z-50 w-[min(500px,calc(100vw-48px))] -translate-x-1/2 -translate-y-1/2 rounded-[var(--ti-radius-lg)] border border-[var(--ti-border-strong)] bg-[var(--ti-surface)] p-6 shadow-[var(--ti-shadow-dialog)]">
      <Dialog.Title class="text-xl font-semibold">{title}</Dialog.Title>
      <Dialog.Description class="mt-3 text-sm leading-6 text-[var(--ti-text-muted)]">{message}</Dialog.Description>
      <div class="mt-6 flex justify-end gap-3">
        <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={onKeepOpen}>Go Back</button>
        {#if action}
          <button type="button" class={`ti-button ${action === "stop" ? "ti-button-danger" : ""}`} disabled={busy} onclick={() => void onPrimary()}>{primaryLabel}</button>
        {/if}
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
