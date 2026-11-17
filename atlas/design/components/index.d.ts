import type * as React from 'react';

/** solid realizes (run, build), outline checks, quiet navigates. */
export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> { variant?: 'solid' | 'outline' | 'quiet' }
export declare function Button(props: ButtonProps): React.ReactElement;

/** One result. The word defaults to Checked, Trap or Open; children replace it. */
export interface StatusProps { state?: 'checked' | 'trap' | 'open'; children?: React.ReactNode; className?: string }
export declare function Status(props: StatusProps): React.ReactElement;

/** outline for target-neutral (Psi) things, solid for realized (Omega) things. */
export interface TileProps { variant?: 'outline' | 'solid'; label?: React.ReactNode; title?: React.ReactNode; children?: React.ReactNode; className?: string }
export declare function Tile(props: TileProps): React.ReactElement;

/** Code or a command on surface. caption names what it is. */
export interface CodeBlockProps { caption?: React.ReactNode; children?: React.ReactNode; className?: string }
export declare function CodeBlock(props: CodeBlockProps): React.ReactElement;

/** Nine results in place order: local, field, parameter, call result, match join, const name, nested machine, another machine, another target. Missing entries are open. */
export interface PlaceGridProps { results?: Array<'checked' | 'trap' | 'open'>; fact?: string; size?: 'full' | 'compact'; className?: string }
export declare function PlaceGrid(props: PlaceGridProps): React.ReactElement;

declare global {
  interface Window {
    Omega: { Button: typeof Button; Status: typeof Status; Tile: typeof Tile; CodeBlock: typeof CodeBlock; PlaceGrid: typeof PlaceGrid };
  }
}
