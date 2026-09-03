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
    game.queue("left");
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
