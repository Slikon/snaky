import { describe, expect, it } from "vitest";
import { SnakeGame } from "./game";

describe("SnakeGame", () => {
  it("moves one cell per tick", () => {
    const game = new SnakeGame(12, 12);
    game.reset();
    const head = game.snake[0];
    game.advance();
    expect(game.snake[0]).toEqual({ x: head.x + 1, y: head.y });
  });

  it("rejects an immediate reverse", () => {
    const game = new SnakeGame(12, 12);
    game.reset();
    expect(game.steer("left")).toBe(false);
    game.advance();
    expect(game.direction).toBe("right");
  });

  it("applies rapid turns as they are entered", () => {
    const game = new SnakeGame(12, 12);
    game.reset();
    const head = game.snake[0];
    const rapidTurns = ["up", "right", "up", "right", "up", "right"] as const;

    rapidTurns.forEach((direction) => {
      expect(game.steer(direction)).toBe(true);
      game.advance();
    });

    expect(game.snake[0]).toEqual({ x: head.x + 3, y: head.y - 3 });
  });

  it("rejects a reversal after steering", () => {
    const game = new SnakeGame(12, 12);
    game.reset();

    expect(game.steer("up")).toBe(true);
    game.advance();
    expect(game.direction).toBe("up");
    expect(game.steer("down")).toBe(false);
    game.advance();
    expect(game.direction).toBe("up");
  });

  it("ignores a repeated direction", () => {
    const game = new SnakeGame(12, 12);
    game.reset();

    expect(game.steer("up")).toBe(true);
    expect(game.steer("up")).toBe(false);
    expect(game.direction).toBe("up");
  });

  it("applies a changed decision without stale turns", () => {
    const game = new SnakeGame(12, 12);
    game.reset();

    const rapidTurns = ["up", "right", "up", "right", "up", "right"] as const;
    rapidTurns.forEach((direction) => {
      game.steer(direction);
      game.advance();
    });

    expect(game.steer("down")).toBe(true);
    game.advance();
    expect(game.direction).toBe("down");
  });

  it("restores the initial direction on reset", () => {
    const game = new SnakeGame(12, 12);
    game.reset();
    game.steer("up");

    game.reset();
    game.advance();

    expect(game.direction).toBe("right");
  });

  it("grows and scores when food is eaten", () => {
    const game = new SnakeGame(12, 12);
    game.reset();
    game.food = { x: game.snake[0].x + 1, y: game.snake[0].y };
    game.advance({ x: 1, y: 1 });
    expect(game.score).toBe(1);
    expect(game.snake).toHaveLength(4);
  });
});
