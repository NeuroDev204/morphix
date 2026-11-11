export interface FileSummary {
  readonly name: string;
  readonly kind: string;
}

export interface WheelItem {
  readonly id: string;
  readonly label: string;
  readonly is_tool: boolean;
  readonly is_suggested: boolean;
}

export interface WheelPayload {
  readonly files: FileSummary[];
  readonly items: WheelItem[];
  readonly total_bytes: number;
}
