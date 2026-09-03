export type Direction = "up" | "down" | "left" | "right";
export type Phase =
  | "waiting"
  | "running"
  | "paused"
  | "attention"
  | "finished"
  | "game-over";

export interface Cell {
  x: number;
  y: number;
}

const vectors: Record<Direction, Cell> = {
  up: { x: 0, y: -1 },
  down: { x: 0, y: 1 },
  left: { x: -1, y: 0 },
  right: { x: 1, y: 0 },
};

const opposites: Record<Direction, Direction> = {
  up: "down",
  down: "up",
  left: "right",
  right: "left",
};

export class SnakeGame {
  readonly columns: number;
  readonly rows: number;
  snake: Cell[] = [];
  food: Cell = { x: 0, y: 0 };
  direction: Direction = "right";
  queuedDirection: Direction = "right";
  phase: Phase = "waiting";
  score = 0;

  constructor(columns = 30, rows = 18) {
    this.columns = columns;
    this.rows = rows;
    this.reset(false);
  }

  reset(started = true): void {
    const center = { x: Math.floor(this.columns / 2), y: Math.floor(this.rows / 2) };
    this.snake = [center, { x: center.x - 1, y: center.y }, { x: center.x - 2, y: center.y }];
    this.direction = "right";
    this.queuedDirection = "right";
    this.score = 0;
    this.food = { x: Math.min(this.columns - 2, center.x + 6), y: center.y };
    this.phase = started ? "running" : "waiting";
  }

  queue(direction: Direction): void {
    if (direction !== opposites[this.direction]) this.queuedDirection = direction;
  }

  togglePause(): void {
    if (this.phase === "running") this.phase = "paused";
    else if (this.phase === "paused") this.phase = "running";
    else if (["waiting", "finished", "game-over"].includes(this.phase)) this.reset();
  }

  advance(nextFood?: Cell): void {
    if (this.phase !== "running") return;
    this.direction = this.queuedDirection;
    const vector = vectors[this.direction];
    const head = this.snake[0];
    const next = { x: head.x + vector.x, y: head.y + vector.y };
    const ate = this.same(next, this.food);
    const occupied = ate ? this.snake : this.snake.slice(0, -1);

    if (
      next.x < 0 ||
      next.x >= this.columns ||
      next.y < 0 ||
      next.y >= this.rows ||
      occupied.some((cell) => this.same(cell, next))
    ) {
      this.phase = "game-over";
      return;
    }

    this.snake.unshift(next);
    if (ate) {
      this.score += 1;
      this.food = nextFood ?? this.randomFreeCell();
    } else {
      this.snake.pop();
    }
  }

  render(): string {
    const cells = Array.from({ length: this.rows }, () => Array(this.columns).fill(" "));
    cells[this.food.y][this.food.x] = "*";
    this.snake.forEach((cell, index) => {
      cells[cell.y][cell.x] = index === 0 ? "@" : "#";
    });
    const edge = `+${"-".repeat(this.columns)}+`;
    return [edge, ...cells.map((row) => `|${row.join("")}|`), edge].join("\n");
  }

  private randomFreeCell(): Cell {
    const free: Cell[] = [];
    for (let y = 0; y < this.rows; y += 1) {
      for (let x = 0; x < this.columns; x += 1) {
        if (!this.snake.some((cell) => cell.x === x && cell.y === y)) free.push({ x, y });
      }
    }
    return free[Math.floor(Math.random() * free.length)] ?? { x: 0, y: 0 };
  }

  private same(left: Cell, right: Cell): boolean {
    return left.x === right.x && left.y === right.y;
  }
}
