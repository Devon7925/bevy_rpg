# RPG Game

by Devon Rutledge

[Backlog](https://github.com/users/Devon7925/projects/1/views/1)

## Game Overview

RPG Game is an AI driven top down RPG game where the player can live and interact freely in a 16th century farming village. The target audience is anyone who wishes to lose themselves in a virtual world and its characters. The game uses a 2D top down perspective and pixel art.

As the player moves through the game they will gain hunger, which they must use food to fix or they will die. They can either either need to farm for food, or convince an NPC to give it to them through manipulation, begging, trading, or work.

## Gameplay

The only built in objective of the game is to not starve. However the player may set alternative objectives for themselves such as helping NPCs with whatever problems they have.

### Getting food

The player will have to get food in order to not starve. In order to get food they can either farm it or get it from NPCS.

If they farm it, they NPCs may spot them and react. For example if you farm on another farmer's property they will may to kick you out, however you could make an agreement with the farmer to keep a portion of the food, trade the food for money, or other emergent possibilities.

If they get it from an NPC, they have many options some of which will be emergent. Here are some ideas:

* Manipulate
* Trade
* Beg
* Ask for it after doing work for them
* Ask for it as a loan
* They may also just decide to give it to you as thanks for doing something for them like repairing their relationship with ther friend

### Other Tasks

RPG Game is meant to be an alive world full of real conflicts that change and adapt. Some conflicts will be built into the original character states. This allows the player to have optional other problems to solve other than just hunger. For example a father might be in an argument with his son which you could resolve either by convincing one of them is right, eliminating the situation surrounding the situation surrounding the argument, or giving them a bigger problem to deal with.  

## Mechanics (Key Section)

The Game will run on a state system. The state contains the following:

* The grid of tiles and their types
* For each NPC and Player
  * Position
  * Direction
  * Hunger level
  * Last thing they said
  * Time since they last spoke
  * Inventory
* For each NPC
  * AI Task
  * Short term memory
  * Long term memory
The state will be updated at a fixed time step with the future state being a result of the previous state and actions. Actions are done by both NPCs and Players and are made of their movement, interaction, and their speech.

Movement can be nothing, moving forward, or a change in direction.
Speech can be nothing or a piece of text representing what they are saying.

Tiles can have a variety of types with internal states:

* Grass
* Building
* Seed(time till grow)
* Crop
* Dirt

The player's action is determined entirely by their control.

The NPC's movement and interaction is determined by their AI Task. Their speech is taken by combining their short and long term memory and feeding it to a LLM to get what they say. The LLM can also affect their AI task. They won't try to speak if they recently spoke. Short term memory is made of recently spoken things and actions they see around them. Long term memory is a occationally summary of what has happened in the past.

## Game World

The world is a small village in 16th century medival europe. The player cannot leave the village and the village is disconnected from the outside world.

## Characters

Characters will be built by putting their personality and history into their initial long term context.

### Examples

You are Theo. A 16th century Farmer living in a small village in medieval europe. You live with your wife Jessica and son Jeff on your own small patch of land. You know your land is small but it has been owned by centuries by your family. Jeff wants to start working on your neighbor Bill's land because it is much bigger, but you want your family to continue farming your ancestral land. You also know you are getting old and tired and will soon need help, especially if you have to support Jessica without help.

You are Jeff. A 16th century Farmer living in a small village in medival europe. You currently live with your parents Theo and Jessica on their small farm. However you know your land is small and will have trouble feeding all three of you so you'd like to move to your neighbor Bill's land in order to stop burdening your family. Theo objects to this due to heritage reasons but you think eating is more important than tradition.

You are Jessica. The wife of a 16th century Farmer living in a small village in medival europe. You live with your husband Theo and son Jeff on a small farm. You know Theo is getting old and will soon need help running the farm but your son Jeff is planning to move to help your neighbor Bill's larger farm at some point. You've tried bring up working the farm yourself but neither Theo nor Jeff seems to listen to you. You otherwise do knitting and housework with most of yor day.

## Interface

The Game is top down pixel art, with a simple pixel art HUD and GUI. The camera will follow the player, keeping them in the middle third of the screen.

The game will have sounds for farming, walking and talking.

### Controls

| Keys | Effect |
|------|--------|
| WASD / Arrows | Movement |
| Space | Interact |
| Enter | Enter chat box / Submit text |
