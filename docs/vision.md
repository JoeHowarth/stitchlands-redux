# Stitchlands: A Colony Sim Where You're Not God

Most colony sims put you in charge of everything. You pick where the base goes. You tell colonists what to build, what to research, who to recruit. The world is empty until you fill it. RimWorld, Dwarf Fortress, Gnomoria, Prison Architect, Oxygen Not Included: they all share this fundamental assumption. You are the entire civilization, starting from nothing.

I want to build something different. I want a game where you drop into an already functioning society and play a role within it. The world does not wait for you to exist. People are already farming, trading, arguing, building, forming alliances, breaking promises. You control one agent or one small group inside this living system. You can grow, expand, scheme, lead. But you are not the world. You are in the world.

## What Exists Today

The colony sim genre has produced some of the best games ever made. RimWorld is probably the most refined example. It nails the moment to moment gameplay: colonists with needs, jobs, skills, moods, relationships. The storyteller system creates emergent drama. The modding community has produced an absurd volume of content. It is genuinely excellent at what it does.

But RimWorld's scope is deliberately small. You manage one colony on one map tile. Your colonists arrive on an empty field and build everything from scratch. The economy is self contained. Growth is fast. Within a few in game years you go from three survivors with a knife to a spacefaring civilization with power armor and antigrain warheads. The rate of return on labor and resources is, frankly, absurd.

Dwarf Fortress goes deeper on simulation fidelity but shares the same basic structure. You embark, you dig, you build, you are the fortress. The wider world exists as a source of migrants and sieges but you do not participate in it as an equal. You are isolated by design.

Factorio and its descendants (Satisfactory, Shapez, Mindustry) solve a different problem entirely. They are about automation and throughput. The world is a resource field and your job is to build the machine that consumes it. Factorio's multiplayer and open world are technically impressive, but the simulation is mechanical, not social.

Settletopia is the closest thing I have seen to what I want to build. It is a colony sim built in Rust with wgpu, with open world exploration, multiple settlements, caravan logistics, and real multiplayer. The developer, Peteris Pakalns, identified the same frustrations I have with traditional colony sims: small maps, no multiplayer, the inability to explore or expand beyond your starting tile. Settletopia solves the spatial problem. You can build multiple settlements, send caravans between them, explore a large procedurally generated world. That is a meaningful step forward.

But Settletopia expands on the spatial and logistical axis. What I want to expand on is the social and structural axis.

## The Core Idea

The game I am building starts with a functioning society. Not an empty map. Not three crash landed survivors. A town, or a region, with people already in it. They have jobs. They have families. They have opinions about each other. They trade with neighboring settlements. They pay taxes to a local lord, or they govern themselves through a council, or they are organized around a religious order. The specific structure depends on the scenario, but the point is that the society already works before you show up.

You play as one person, or one family, or one organization within this society. You are a participant. You can become a farmer, a merchant, a warlord, a priest, a mayor. You can build a trading company. You can raise an army. You can try to become king. But the world does not revolve around you. Other people are doing their own things, pursuing their own goals, forming their own relationships.

The key analogy is this: a well managed company can grow much faster than the GDP of the country it operates in. Amazon grew at 20% per year while the US economy grew at 2%. But Amazon could not make the US economy grow at 20%. The macro system has its own constraints. Your local optimization can dramatically outperform the average, but you cannot break the system's overall speed limit.

In game terms this means: you can expand your holdings, build more efficiently than your neighbors, accumulate wealth and power faster than the baseline. But the overall society grows slowly. Resources are finite and regionally distributed. Population grows at realistic rates. Technology advances gradually. You are playing within constraints, not above them.

## Social Graphs

RimWorld has a relationship system. Colonists can be friends, lovers, rivals. They have opinions of each other based on interactions. It is functional but shallow. Every colonist is essentially equal in social standing. There are no hierarchies, no obligations, no formal structures beyond "these people live in the same colony."

What I want is an explicit social graph. Think of it less like a game mechanic and more like an actual network of human relationships, compressed and abstracted for gameplay purposes.

Every pawn in the world has connections to other pawns. These connections have types: family, friendship, professional, feudal, commercial, religious, antagonistic. They have strength, which changes over time based on interaction. Two people who work together every day build a stronger professional bond than two people who met once at a market. People who grew up in the same settlement share a baseline cultural connection. People who share a religion have a tie through that. People who fought a war together have a bond that civilians do not understand.

These connections are not just flavor text. They determine how the world actually works. When a lord calls his vassals to war, the vassals who have strong loyalty ties show up. The ones with weak ties might not. When a merchant needs a loan, they go to someone in their commercial network. When a dispute arises, it gets resolved through the social graph: who mediates, whose opinion matters, who gets excluded.

Arguments happen. Grudges form. Blood feuds persist across generations. A business deal gone wrong can poison a relationship for years. A marriage can bind two families together and reshape the political landscape of an entire settlement. None of this needs to be scripted. It emerges from the interactions between pawns within the social graph.

## Economics

RimWorld's economy is essentially unlimited. You mine steel, you build things, you research more things, you build better things. Resources are abundant. Labor is the only real constraint, and even that dissolves as you recruit more colonists. The economy has no friction, no scarcity that cannot be overcome, no regional specialization that matters.

I want production chains that are bounded by geography. Iron ore exists in some places and not others. Good farmland is not everywhere. Timber requires forests. Salt comes from the coast or from specific mineral deposits. This is not a new idea in games, but I want to take it seriously as a structural constraint rather than a minor inconvenience.

Not every settlement can produce everything it needs. This means trade is not optional. It is a requirement for any settlement beyond subsistence level. And trade means relationships, contracts, trust, routes, security, and all the complexity that comes with actual commerce.

Growth is slow. Building a house takes weeks, not hours. Clearing land for farming takes a season. Training a skilled craftsman takes years. An economy grows through capital accumulation and specialization, not through a research tree that unlocks magical productivity multipliers. When your settlement produces more grain than it consumes, the surplus gets traded, invested, or stored. It does not disappear into a score counter.

This slower pace changes what the game is about. You are not racing to build the biggest base before the next raid. You are managing a long term trajectory. The decisions that matter are not "what do I build next" but "what relationships do I invest in, what trade routes do I establish, what political alliances protect my interests over the next decade."

## Scale

The world needs to be large enough that the player is genuinely a small part of it. Not a god managing an ant farm, but an ant in a world of ants who happens to be particularly ambitious and capable.

This means simulating a lot of pawns. Hundreds or thousands of characters, most of whom the player never directly interacts with. The background simulation needs to be cheap. You cannot run a full RimWorld style needs/mood/pathfinding tick for every NPC in the world every frame. The simulation has to be layered: detailed simulation for pawns near the player or in settlements the player controls, and a much cheaper abstract simulation for everyone else.

Settletopia solved a version of this problem. Their persistent creature simulation keeps entities alive off screen, running simplified behavior rather than full AI. The same principle applies here but extended to social and economic behavior. A distant settlement does not need per pawn pathfinding. It needs to know: how much grain did it produce this season, did it meet its tax obligations, did any notable social events occur, is it growing or shrinking.

The map itself needs to be large. Multiple biomes, multiple settlements, meaningful distances between places. Travel takes time. Information travels slower than the player. You might hear about a war in the north weeks after it started. This is not about realism for its own sake. It creates interesting gameplay because it means you cannot micromanage everything. You have to delegate, trust, build systems, and accept imperfect information.

## Technical Foundation

The project is built in Rust with wgpu for rendering. Right now the codebase is about 9,500 lines. It can parse RimWorld's XML definition files, resolve textures from both loose PNG files and packed Unity asset bundles, compose layered pawn sprites (body, head, hair, beard, apparel with correct z ordering), and render them in an interactive window with mouse driven selection and pathfinding.

The immediate technical approach is to use RimWorld's assets directly during private development. RimWorld ships with a large volume of high quality 2D sprites: terrain, buildings, items, plants, animals, clothing, body parts. By parsing the game's XML definitions and extracting textures from its Unity asset bundles, I can get a complete visual foundation without creating a single art asset. This sidesteps the content cold start problem that kills many indie projects before they produce anything playable.

For public release, the game will use assets from open source RimWorld mods. The RimWorld modding community has produced enormous amounts of freely licensed content. The XML definition format will remain compatible, so switching from base game assets to mod assets is a data change, not a code change.

The rendering pipeline uses instanced sprite batching with wgpu. Sprites are grouped by texture and sorted by depth. The scene is split into static layers (terrain, buildings, items) and dynamic layers (pawns) so that the static world does not need to be resubmitted every frame. This architecture should scale to large maps without fundamental changes, though there will obviously be work around culling, level of detail, and streaming.

The current runtime is a fixed timestep tick loop with A* pathfinding on a tile grid. Pawns have positions, facing directions, movement speeds, and path progress. There is a basic interaction system: click to select, click to issue movement commands. It is the absolute minimum viable foundation for a simulation game, and everything above it still needs to be built.

## What Comes Next

The immediate plan is to faithfully recreate a subset of RimWorld's core gameplay. Not all of it. Not the full storyteller, not the research tree, not the faction system. Just the basics: pawns with needs (hunger, rest, comfort, mood), a job system (haul, construct, grow, harvest, cook, clean), construction of basic structures, and the day/night cycle that drives scheduling.

Getting these fundamentals right matters because everything else builds on top of them. The social graph means nothing if pawns do not have daily routines that create opportunities for interaction. The economic model means nothing if there is no production system to model. Regional scarcity means nothing if settlements cannot actually produce and consume goods.

Once the RimWorld baseline is solid, the social and economic layers go on top. The social graph. The relationship dynamics. The hierarchical structures (lords, vassals, councils, guilds, families). The bounded economy with regional resources and real production chains. The background simulation that keeps the world alive beyond the player's immediate view.

Then the hard part: making it all feel like a coherent game and not a spreadsheet. The moment to moment gameplay still needs to be a colony sim. You still watch your people go about their days, build things, interact with each other. The higher level systems (politics, economics, social dynamics) should create the conditions and constraints for that daily life, not replace it. The player should feel like they are living inside a society, not managing one from above.

That is what I am building toward. It is going to take a while.
