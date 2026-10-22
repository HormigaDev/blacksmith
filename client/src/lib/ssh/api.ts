/**
 * Contrato con los comandos de Rust del túnel SSH (client/src-tauri/src/commands.rs). La llave
 * privada viaja hacia Rust una sola vez (al guardarla) y nunca vuelve: los comandos solo devuelven
 * resúmenes (algoritmo y huella).
 */
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import { toTunnelError } from './errors';

export interface SshTarget {
    host: string;
    /** 22 si se omite. */
    port?: number;
    username: string;
}

export type KeySource =
    /** La primera de `~/.ssh/{id_ed25519,id_ecdsa,id_rsa}`. */
    | { kind: 'default' }
    /** Ruta elegida con el FilePicker. */
    | { kind: 'file'; path: string }
    /** Contenido pegado. */
    | { kind: 'inline'; content: string };

export interface KeySummary {
    algorithm: string;
    fingerprint: string;
    encrypted: boolean;
}

/**
 * Servidor configurado (sin secretos).
 * - `ssh`: un VPS al que se llega por túnel. `id` es `usuario@host:puerto`, normalizado por Rust.
 * - `local`: este equipo; sin túnel ni llave. `id` es `local:<puerto>` y `port` es el del agente.
 */
export interface ServerProfile {
    id: string;
    kind: 'ssh' | 'local';
    host: string;
    port: number;
    username: string;
}

/** Espejo de `shared::AgentStatus`. */
export interface AgentStatus {
    name: string;
    version: string;
    status: 'ready';
}

/** Puerto del agente por defecto (`BLACKSMITH_BIND_ADDR`). */
export const AGENT_PORT = 4040;

export interface SavedServer {
    server: ServerProfile;
    key: KeySummary;
}

export interface KeyInfo extends KeySummary {
    passphraseSaved: boolean;
}

export interface TunnelInfo {
    targetId: string;
    /** Puerto de 127.0.0.1 en este equipo que llega al agente. */
    localPort: number;
    remotePort: number;
    /** Lo que respondió el agente al conectar. */
    agent: AgentStatus | null;
}

export interface TunnelClosed {
    targetId: string;
    reason: 'requested' | 'connectionLost';
}

export const TUNNEL_CLOSED_EVENT = 'tunnel://closed';

async function call<T>(command: string, args: Record<string, unknown>): Promise<T> {
    try {
        return await invoke<T>(command, args);
    } catch (error) {
        throw toTunnelError(error);
    }
}

/**
 * Valida la llave (y su passphrase), la guarda cifrada y registra el servidor. El servidor queda
 * guardado aunque luego no se pueda conectar.
 */
export function saveKey(
    target: SshTarget,
    source: KeySource,
    options: { passphrase?: string; rememberPassphrase?: boolean } = {},
): Promise<SavedServer> {
    return call('ssh_save_key', {
        target,
        source,
        passphrase: options.passphrase || null,
        rememberPassphrase: options.rememberPassphrase ?? false,
    });
}

/** Servidores configurados, en orden de alta. Fuera de Tauri (navegador) no hay ninguno. */
export async function listServers(): Promise<ServerProfile[]> {
    if (!isTauri()) return [];
    return call('servers_list', {});
}

/** Quita el servidor (si es SSH, cierra su túnel y borra su llave). Acción destructiva: confirmar antes. */
export function removeServer(id: string): Promise<boolean> {
    return call('server_remove', { id });
}

/** Guarda el nuevo orden. `ids` debe contener exactamente los servidores guardados. */
export function reorderServers(ids: readonly string[]): Promise<ServerProfile[]> {
    return call('servers_reorder', { ids });
}

/** Registra este equipo como servidor: sin túnel, directo al agente en 127.0.0.1:<puerto>. */
export function addLocalServer(port: number = AGENT_PORT): Promise<ServerProfile> {
    return call('server_add_local', { port });
}

/** Comprueba el agente local (solo 127.0.0.1). */
export function localAgentStatus(port: number = AGENT_PORT): Promise<AgentStatus> {
    return call('local_agent_status', { port });
}

/** Destino SSH de un servidor configurado. */
export function targetOf(server: ServerProfile): Required<SshTarget> {
    return { host: server.host, port: server.port, username: server.username };
}

export function keyInfo(target: SshTarget): Promise<KeyInfo | null> {
    return call('ssh_key_info', { target });
}

/** Acción destructiva: confirmar con `ConfirmDialog` antes. */
export function forgetKey(target: SshTarget): Promise<boolean> {
    return call('ssh_forget_key', { target });
}

/** Acción destructiva: confirmar con `ConfirmDialog` antes. */
export function forgetHostKey(target: SshTarget): Promise<boolean> {
    return call('ssh_forget_host', { target });
}

export function openTunnel(
    target: SshTarget,
    options: { passphrase?: string; localPort?: number } = {},
): Promise<TunnelInfo> {
    return call('tunnel_open', {
        target,
        passphrase: options.passphrase || null,
        localPort: options.localPort ?? null,
    });
}

/** Confirma la llave del servidor recién presentada. Solo acepta esa huella. */
export function trustHostKey(target: SshTarget, fingerprint: string): Promise<void> {
    return call('tunnel_trust_host', { target, fingerprint });
}

export function closeTunnel(target: SshTarget): Promise<boolean> {
    return call('tunnel_close', { target });
}

export function tunnelStatus(target: SshTarget): Promise<TunnelInfo | null> {
    return call('tunnel_status', { target });
}

/** Avisa cuando un túnel se cierra o se pierde. Fuera de Tauri no hace nada. */
export async function onTunnelClosed(handler: (event: TunnelClosed) => void): Promise<UnlistenFn> {
    if (!isTauri()) return () => {};
    return listen<TunnelClosed>(TUNNEL_CLOSED_EVENT, (event) => handler(event.payload));
}
