import { Router, type IRouter } from "express";
import healthRouter from "./health";
import dashboardRouter from "./dashboard";
import searchRouter from "./search";
import filesRouter from "./files";
import archiveRouter from "./archive";
import backupRouter from "./backup";
import workspaceRouter from "./workspace";
import convertRouter from "./convert";
import devRouter from "./dev";

const router: IRouter = Router();

router.use(healthRouter);
router.use(dashboardRouter);
router.use(searchRouter);
router.use(filesRouter);
router.use(archiveRouter);
router.use(backupRouter);
router.use(workspaceRouter);
router.use(convertRouter);
router.use(devRouter);

export default router;
